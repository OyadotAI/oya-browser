/**
 * Screen sharing (getDisplayMedia): a source chosen on every request, after the same
 * gate and Allow dialog as the camera, remembered per profile. Without a
 * handler Electron refuses every request and a call's Present button fails
 * silently. No system picker: Electron skips this handler entirely when the
 * OS picker is used, so no agent-tab, governance or origin rule would run.
 */
import type { Session, WebContents, MediaAccessPermissionRequest } from 'electron';
import type { AppServices } from './services.ts';
import { shareScreenPrompt } from './media-prompts.ts';
import { ALLOW_MEDIA_CHOICE } from './constants.ts';
import type { ShareVideo } from '../screen-share/sources.ts';

/** The gate and dialog (through mediaPermissions), the profile, the grants, and the capturer. */
type Deps = Pick<
  AppServices,
  'electron' | 'mediaPermissions' | 'persona' | 'sitePermissions' | 'screenPicker' | 'shareIndicator'
>;

/** Electron's exact handler type, without re-declaring its unions. */
type DisplayHandler = NonNullable<Parameters<Session['setDisplayMediaRequestHandler']>[0]>;
/** One request: the frame asking and its origin. */
type DisplayRequest = Parameters<DisplayHandler>[0];
/** What the handler hands the page: a screen source, or nothing. */
type Streams = Parameters<Parameters<DisplayHandler>[1]>[0];

/** Human-approved screen capture, scoped to the requesting document's profile. */
export class ScreenShare {
  /** Services. */
  private readonly deps: Deps;
  /** Testable platform branch for the macOS Screen Recording setting. */
  private readonly platform: string;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps, platform = process.platform) {
    this.deps = deps;
    this.platform = platform;
  }

  /** Native display and legacy capture both preflight as media without devices; ask here so neither can bypass consent. */
  request(contents: WebContents, details: MediaAccessPermissionRequest): Promise<boolean> {
    const { gate } = this.deps.mediaPermissions;
    const origin =
      details?.isMainFrame === true && gate.eligible(contents, details.securityOrigin || details.requestingUrl);
    if (!origin) return Promise.resolve(false);
    return gate.locked(() => this.approve(contents, origin));
  }

  /** Installs the handler on a profile's session; every way it can fail answers the page nothing. */
  install(ses: Pick<Session, 'setDisplayMediaRequestHandler'>): void {
    ses.setDisplayMediaRequestHandler(
      (request, callback) => void this.answer(request).then(callback, () => callback(null)),
    );
  }

  /** The request's tab, only when the top document itself is asking. */
  private requester(request: DisplayRequest): WebContents | null {
    const contents = request.frame && this.deps.electron.webContents.fromFrame(request.frame);
    return contents && request.frame === contents.mainFrame ? contents : null;
  }

  /** The gate, site consent unless remembered, then a fresh explicit source choice. */
  private async answer(request: DisplayRequest): Promise<Streams | null> {
    const contents = this.requester(request);
    const origin = this.deps.mediaPermissions.gate.eligible(contents, request.securityOrigin);
    if (!contents || !origin) return null;
    const generation = this.deps.mediaPermissions.gate.watch(contents);
    const partition = this.deps.persona.partitionName();
    const allowed = await this.deps.mediaPermissions.gate.locked(() => this.approve(contents, origin));
    return allowed ? this.currentScreen(contents, generation, partition) : null;
  }

  /** Source enumeration can prompt macOS too; recheck the document and owner before handing pixels to the page. */
  private async currentScreen(contents: WebContents, generation: number, partition: string): Promise<Streams | null> {
    const current = () =>
      this.deps.mediaPermissions.gate.current(contents, generation) && this.deps.persona.partitionName() === partition;
    const streams = await this.deps.screenPicker.pick(contents, new URL(contents.getURL()).origin, current);
    if (!streams || !current()) return null;
    // Marked only once a source is handed over: the person can still cancel the picker.
    this.deps.mediaPermissions.gate.markLive(contents);
    this.deps.shareIndicator?.started(streams.video as ShareVideo, contents);
    return streams;
  }

  /** Site consent (remembered as soon as given, as the camera's is), the document and control rechecked after the wait, then the macOS setting. */
  private async approve(contents: WebContents, origin: string): Promise<boolean> {
    const { gate } = this.deps.mediaPermissions;
    const partition = this.deps.persona.partitionName();
    const generation = gate.watch(contents);
    const remembered = this.deps.sitePermissions.has(partition, origin, 'screen');
    if (!remembered && !(await gate.allowedBy(shareScreenPrompt(origin), ALLOW_MEDIA_CHOICE))) return false;
    if (!gate.current(contents, generation) || this.deps.persona.partitionName() !== partition) return false;
    if (!remembered) this.deps.sitePermissions.grant(partition, origin, ['screen']);
    return this.confirmOS(contents, generation, partition);
  }

  /** Request native enumeration before refusing: otherwise macOS may never register Oya for permission. */
  private async confirmOS(contents: WebContents, generation: number, partition: string): Promise<boolean> {
    if (this.osDenied()) await this.deps.screenPicker.requestOSPermission();
    const { gate } = this.deps.mediaPermissions;
    if (!gate.current(contents, generation) || this.deps.persona.partitionName() !== partition) return false;
    return this.osDenied() ? gate.osDenied(contents, generation, true) : true;
  }

  /** macOS Screen Recording turned off for Oya in System Settings; elsewhere the OS has no such switch. */
  private osDenied(): boolean {
    const denied =
      this.platform === 'darwin' &&
      ['denied', 'restricted'].includes(this.deps.electron.systemPreferences.getMediaAccessStatus('screen'));
    return denied;
  }
}
