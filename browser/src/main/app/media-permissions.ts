/**
 * Human-approved microphone and camera access. A site asks once per profile;
 * Allow is remembered on disk (site-permissions.ts), so a meeting is not a new
 * prompt every time. The gate (media-gate.ts) still runs on every check and
 * request, so a remembered grant never reaches a tab an agent drives, a
 * governed browser or a background tab, and macOS consent is confirmed each time.
 */
import type { WebContents, MediaAccessPermissionRequest, PermissionCheckHandlerHandlerDetails } from 'electron';
import type { AppServices } from './services.ts';
import { MediaGate } from './media-gate.ts';
import { allowDevicesPrompt, type DeviceKind, type MediaDenial } from './media-prompts.ts';
import { ALLOW_MEDIA_CHOICE } from './constants.ts';

/** The gate's services, the profile the grants are keyed by, and where they are kept. */
type Deps = Pick<
  AppServices,
  'electron' | 'shell' | 'tabs' | 'agentTab' | 'governance' | 'persona' | 'sitePermissions'
>;

/** Whether `type` is a device a site can ask for. */
const isDevice = (type: unknown): type is DeviceKind => type === 'audio' || type === 'video';
/** The original request carries its complete asynchronous authorization fence. */
interface DeviceApproval {
  /** Native requesting tab. */ contents: WebContents;
  /** Secure requesting origin. */ origin: string;
  /** Only requested microphone/camera devices. */ types: DeviceKind[];
  /** Original window, session, profile, document and ownership. */ current(): boolean;
}

/** Empty or unknown media types must not turn into permission for everything. */
function requestedMedia(details: MediaAccessPermissionRequest): DeviceKind[] {
  const types = details.mediaTypes;
  return types?.length && types.every(isDevice) ? [...new Set(types)] : [];
}

/** Site consent, remembered per profile, then OS consent, every time. */
export class MediaPermissions {
  /** The rules and the dialog, shared with screen sharing. */
  readonly gate: MediaGate;
  /** The profile and the remembered grants. */
  private readonly deps: Deps;
  /** Testable platform branch for macOS privacy consent. */
  private readonly platform: string;

  /** Native OS consent is requested only after site consent. */
  constructor(deps: Deps, platform = process.platform) {
    this.deps = deps;
    this.platform = platform;
    this.gate = new MediaGate(deps);
  }

  /** The profile whose grants apply right now. */
  private partition(): string {
    return this.deps.persona.partitionName();
  }

  /** Permission checks grant only what this profile remembered for this site, and only to a tab the person may use now. Never prompts. */
  check(contents: WebContents | null, origin: string, details: PermissionCheckHandlerHandlerDetails): boolean {
    const site = details && this.gate.eligible(contents, details.securityOrigin || origin);
    const type = details?.mediaType;
    return (
      !!site && isDevice(type) && this.deps.sitePermissions.has(this.partition(), site, type) && this.osGranted(type)
    );
  }

  /** A remembered site decision cannot report a device as available while macOS blocks it. */
  private osGranted(type: DeviceKind): boolean {
    return (
      this.platform !== 'darwin' ||
      this.deps.electron.systemPreferences.getMediaAccessStatus(type === 'audio' ? 'microphone' : 'camera') ===
        'granted'
    );
  }

  /** Async permission requests always settle to a boolean, including OS/dialog failures. */
  request(contents: WebContents, details: MediaAccessPermissionRequest): Promise<boolean> {
    const origin = details && this.gate.eligible(contents, details.securityOrigin || details.requestingUrl);
    const types = details ? requestedMedia(details) : [];
    if (!origin || !types.length) return Promise.resolve(false);
    const current = this.gate.guard(contents, origin);
    const key = this.gate.decisionKey(contents, origin, types);
    return this.gate.locked(current, () => this.approve({ contents, origin, types, current }), key);
  }

  /**
   * Site consent, then OS consent, with control and the document rechecked after
   * each wait. The person's Allow is remembered as soon as it is given, before
   * the OS is asked: site and OS consent are separate decisions (as in Chrome),
   * and a device macOS blocks must not turn into the Allow box on every retry.
   */
  private async approve({ contents, origin, types, current }: DeviceApproval): Promise<boolean> {
    const remembered = this.remembered(origin, types);
    if (!remembered && !(await this.gate.allowedBy(allowDevicesPrompt(origin, types), ALLOW_MEDIA_CHOICE, contents)))
      return false;
    if (!current()) return false;
    if (!remembered) this.deps.sitePermissions.grant(this.partition(), origin, types);
    return this.confirmDevices(contents, current, types);
  }

  /** OS consent cannot grant a replacement document or profile, even after the site decision was saved. */
  private async confirmDevices(contents: WebContents, current: () => boolean, types: DeviceKind[]): Promise<boolean> {
    const failure = await this.osConsent(types);
    if (!current()) return false;
    return failure ? this.gate.osDenied(contents, current, failure) : this.gate.markLive(contents);
  }

  /** Whether this profile already allowed every requested device on this site. */
  private remembered(origin: string, types: DeviceKind[]): boolean {
    return types.every((type) => this.deps.sitePermissions.has(this.partition(), origin, type));
  }

  /** macOS also requires system-level permission (answered at once when already granted); other platforms enforce it themselves. */
  private async osConsent(types: DeviceKind[]): Promise<MediaDenial | null> {
    if (this.platform !== 'darwin') return null;
    for (const type of types) {
      const failure = await this.osDevice(type);
      if (failure) return failure;
    }
    return null;
  }
  /** The observed status, not an exception or false result alone, establishes OS denial. */
  private async osDevice(type: DeviceKind): Promise<MediaDenial | null> {
    const device = type === 'audio' ? 'microphone' : 'camera';
    try {
      if (await this.deps.electron.systemPreferences.askForMediaAccess(device)) return null;
      return { device: type, status: this.deps.electron.systemPreferences.getMediaAccessStatus(device) };
    } catch {
      return { device: type, status: 'unknown' };
    }
  }
}
