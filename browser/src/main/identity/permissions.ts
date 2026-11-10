/**
 * What a page may do without asking. Electron grants every permission unasked:
 * any page could open the camera and microphone, read the location, and list
 * the machine's real devices by name, and `navigator.permissions` said
 * "granted" to everything, which no browser does and detectors read in one call.
 *
 * Microphone and camera requests use explicit site and OS consent, and notifications
 * the person's remembered Allow or Block (notification-permissions.ts). Other
 * sensitive permissions remain refused; ordinary unasked permissions are allowed. The page injection
 * reports a refused permission as Chrome's "prompt" (anonymity/stealth.js).
 * External app requests remain denied here, but may be delegated to a separate
 * human-confirmed desktop handoff to an installed protocol handler.
 */
import type { Session, WebContents, MediaAccessPermissionRequest, PermissionRequest } from 'electron';
import type { MediaPermissions } from '../app/media-permissions.ts';
import type { NotificationPermissions } from '../app/notification-permissions.ts';
import type { ScreenShare } from '../app/screen-share.ts';
import { UNASKED } from './constants.ts';

/** Whether `permission` is one Chrome grants without asking. */
export const unasked = (permission: string): boolean => UNASKED.includes(permission);

/** A denied external protocol may be offered to the explicit native confirmation service. */
export type ExternalAppRequest = (url: string, contents: WebContents) => void;

/** Only the session permission hooks are required; the display-media hook when screen share is installed. */
type PermissionSession = Pick<Session, 'setPermissionRequestHandler' | 'setPermissionCheckHandler'> &
  Partial<Pick<Session, 'setDisplayMediaRequestHandler'>>;

/** The consent services a session has, in order: microphone and camera, screen share, notifications. */
type ConsentArgs = [media?: MediaPermissions, screenShare?: ScreenShare, notifications?: NotificationPermissions];

/** Media, screen share and notifications have explicit consent paths; other sensitive permissions remain denied. Governance overrides all. */
export function installPermissions(
  ses: PermissionSession,
  externalApp?: ExternalAppRequest,
  ...consent: ConsentArgs
): void {
  const [media, screenShare, notifications] = consent;
  const screen = installScreenShare(ses, screenShare) ? screenShare : undefined;
  ses.setPermissionRequestHandler(permissionRequest({ externalApp, media, screen, notifications }));
  ses.setPermissionCheckHandler(permissionCheck(media, notifications));
}

/** The exact native check callback type. */
type CheckHandler = NonNullable<Parameters<Session['setPermissionCheckHandler']>[0]>;
/** Media and notifications read as granted only when this profile remembered it for this site; everything else as Chrome has it. */
function permissionCheck(media?: MediaPermissions, notifications?: NotificationPermissions): CheckHandler {
  return (contents, permission, origin, details) => {
    if (permission === 'media') return !!media?.check(contents, origin, details);
    if (permission === 'notifications') return !!notifications?.check(contents, origin);
    return unasked(permission);
  };
}

/** Screen share needs the display-media hook, which a bare permission session (tests, dialogs) may not have. */
function installScreenShare(ses: PermissionSession, screenShare?: ScreenShare): boolean {
  if (!screenShare || !ses.setDisplayMediaRequestHandler) return false;
  screenShare.install(ses as Session);
  return true;
}

/** The exact native request callback type, without re-declaring Electron's union. */
type RequestHandler = NonNullable<Parameters<Session['setPermissionRequestHandler']>[0]>;

/** Both display-media and legacy desktop capture use an empty device list; require screen consent before either can proceed. */
function requestMedia(
  contents: WebContents,
  details: MediaAccessPermissionRequest,
  media: MediaPermissions,
  screen?: ScreenShare,
): Promise<boolean> {
  const display = screen && Array.isArray(details?.mediaTypes) && !details.mediaTypes.length;
  return display ? screen.request(contents, details) : media.request(contents, details);
}

/** Explicit consent paths installed for this exact session. */
interface PermissionServices {
  /** Confirm external application handoffs with the person. */
  externalApp?: ExternalAppRequest;
  /** Camera and microphone site and OS consent. */
  media?: MediaPermissions;
  /** Screen consent, present only when the native display handler is installed. */
  screen?: ScreenShare;
  /** Remembered notification consent; absent in private and dialog sessions, which then refuse. */
  notifications?: NotificationPermissions;
}

/** Settle every native callback, including unavailable services and failed consent prompts. */
function answerPermission(callback: (allowed: boolean) => void, answer?: Promise<boolean>): void {
  if (!answer) return callback(false);
  void answer.then(callback, () => callback(false));
}

/** Only native external-protocol details may initiate a separate human confirmation. */
function offerExternal(services: PermissionServices, contents: WebContents, details: PermissionRequest): void {
  if (details && 'externalURL' in details && typeof details.externalURL === 'string')
    services.externalApp?.(details.externalURL, contents);
}

/** What Electron tells a request handler about the request, the handler's last argument. */
type RequestDetails = Parameters<RequestHandler> extends [unknown, unknown, unknown, infer D] ? D : never;

/** A permission with a consent path: answers the person's decision, or nothing when the session has no such service. */
type Consent = (
  contents: WebContents,
  details: RequestDetails,
  services: PermissionServices,
) => Promise<boolean> | undefined;

/** The permissions that ask the person (or remember what they said); every other one is answered by `unasked`. */
const CONSENT: Record<string, Consent> = {
  media: (contents, details, { media, screen }) =>
    media && requestMedia(contents, details as MediaAccessPermissionRequest, media, screen),
  'display-capture': (contents, details, { screen }) => screen?.request(contents, details),
  notifications: (contents, details, { notifications }) => notifications?.request(contents, details?.requestingUrl),
};

/** Consent permissions go to their service; external apps may be offered to the person; all other rules stay unchanged. */
function permissionRequest(services: PermissionServices): RequestHandler {
  return (contents, permission, callback, details) => {
    if (Object.hasOwn(CONSENT, permission))
      return answerPermission(callback, CONSENT[permission](contents, details, services));
    callback(unasked(permission));
    if (permission === 'openExternal') offerExternal(services, contents, details);
  };
}
