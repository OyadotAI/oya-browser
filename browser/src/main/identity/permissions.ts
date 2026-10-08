/**
 * What a page may do without asking. Electron grants every permission unasked:
 * any page could open the camera and microphone, read the location, and list
 * the machine's real devices by name, and `navigator.permissions` said
 * "granted" to everything, which no browser does and detectors read in one call.
 *
 * Microphone and camera requests use explicit site and OS consent. Other
 * sensitive permissions remain refused; ordinary unasked permissions are allowed. The page injection
 * reports a refused permission as Chrome's "prompt" (anonymity/stealth.js).
 * External app requests remain denied here, but may be delegated to a separate
 * human-confirmed, allowlisted desktop handoff.
 */
import type { Session, WebContents } from 'electron';
import type { MediaPermissions } from '../app/media-permissions.ts';
import { UNASKED } from './constants.ts';

/** Whether `permission` is one Chrome grants without asking. */
export const unasked = (permission: string): boolean => UNASKED.includes(permission);

/** A denied external protocol may be offered to the explicit native confirmation service. */
export type ExternalAppRequest = (url: string, contents: WebContents) => void;

/** Only the session permission hooks are required. */
type PermissionSession = Pick<Session, 'setPermissionRequestHandler' | 'setPermissionCheckHandler'>;

/** Media has an explicit consent path; other sensitive permissions remain denied. Governance overrides both. */
export function installPermissions(
  ses: PermissionSession,
  externalApp?: ExternalAppRequest,
  media?: MediaPermissions,
): void {
  ses.setPermissionRequestHandler(permissionRequest(externalApp, media));
  ses.setPermissionCheckHandler((contents, permission, origin, details) =>
    permission === 'media' ? !!media?.check(contents, origin, details) : unasked(permission),
  );
}

/** The exact native request callback type, without re-declaring Electron's union. */
type RequestHandler = NonNullable<Parameters<Session['setPermissionRequestHandler']>[0]>;
/** Sensitive media uses explicit consent; all other permission rules stay unchanged. */
function permissionRequest(externalApp?: ExternalAppRequest, media?: MediaPermissions): RequestHandler {
  return (contents, permission, callback, details) => {
    if (permission === 'media' && media)
      return void media.request(contents, details).then(callback, () => callback(false));
    callback(unasked(permission));
    if (permission === 'openExternal' && details && 'externalURL' in details && typeof details.externalURL === 'string')
      externalApp?.(details.externalURL, contents);
  };
}
