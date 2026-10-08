/** Human-approved microphone/camera access, scoped to the requesting document. */
import type {
  WebContents,
  MediaAccessPermissionRequest,
  PermissionCheckHandlerHandlerDetails,
  MessageBoxOptions,
} from 'electron';
import type { AppServices } from './services.ts';
import { ALLOW_MEDIA_CHOICE } from './constants.ts';
/** Sensitive access uses native dialogs and the normal control boundary. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'tabs' | 'control' | 'governance'>;
/** Only these two media kinds may be requested. */
type MediaKind = 'audio' | 'video';
/** Approval belongs to one document, not an entire browser profile. */
interface Grant {
  /** Full page address fences navigation while a native prompt is open. */
  url: string;
  /** Device types explicitly approved by the person and operating system. */
  types: Set<MediaKind>;
}
/** HTTPS origins are displayed rather than private paths and query strings. */
function mediaOrigin(raw: string): string | null {
  const url = URL.parse(raw);
  return url?.protocol === 'https:' && !url.username && !url.password ? url.origin : null;
}
/** Empty or unknown media types must not turn into permission for everything. */
function requestedMedia(details: MediaAccessPermissionRequest): MediaKind[] {
  const types = details.mediaTypes;
  return types?.length && types.every((type) => type === 'audio' || type === 'video') ? [...new Set(types)] : [];
}
/** No permanent grants: reload, navigation and tab destruction revoke checks. */
export class MediaPermissions {
  /** Native APIs, active tab and control policy. */
  private readonly deps: Deps;
  /** Testable platform branch for macOS privacy consent. */
  private readonly platform: string;
  /** Approvals never reach disk or another profile's webContents. */
  private readonly grants = new WeakMap<WebContents, Grant>();
  /** Navigation generations fence reloads even when the URL does not change. */
  private readonly generations = new WeakMap<WebContents, number>();
  /** Prevent a page from stacking permission dialogs. */
  private pending = false;
  /** Native OS consent is requested only after site consent. */
  constructor(deps: Deps, platform = process.platform) {
    this.deps = deps;
    this.platform = platform;
  }
  /** Refuse background tabs, remote agents and managed runtimes. */
  private allowed(contents: WebContents | null): contents is WebContents {
    return (
      !!contents &&
      !contents.isDestroyed() &&
      !this.deps.governance.configuration &&
      this.deps.control.snapshot().interactive &&
      this.deps.tabs.getActiveView()?.webContents === contents
    );
  }
  /** A same-origin secure document is the only eligible requester. */
  private origin(contents: WebContents, raw: string): string | null {
    const origin = mediaOrigin(raw);
    return origin && origin === mediaOrigin(contents.getURL()) ? origin : null;
  }
  /** Async permission requests always settle to a boolean, including OS/dialog failures. */
  async request(contents: WebContents, details: MediaAccessPermissionRequest): Promise<boolean> {
    if (!this.allowed(contents) || this.pending || !details) return false;
    const origin = this.origin(contents, details.securityOrigin || details.requestingUrl);
    const types = requestedMedia(details);
    if (!origin || !types.length) return false;
    this.pending = true;
    return this.approve(contents, origin, types)
      .catch(() => false)
      .finally(() => this.settled());
  }
  /** Always release the prompt lock, including exceptions and denials. */
  private settled(): void {
    this.pending = false;
  }
  /** Permission checks cannot grant devices that have not been explicitly approved. */
  check(contents: WebContents | null, origin: string, details: PermissionCheckHandlerHandlerDetails): boolean {
    if (!this.allowed(contents) || !details || !this.origin(contents, details.securityOrigin || origin)) return false;
    const grant = this.grants.get(contents),
      type = details.mediaType;
    return (
      !!grant && grant.url === contents.getURL() && (type === 'audio' || type === 'video') && grant.types.has(type)
    );
  }
  /** Recheck source and control after every OS prompt before granting capture. */
  private async approve(contents: WebContents, origin: string, types: MediaKind[]): Promise<boolean> {
    const generation = this.watch(contents);
    const result = await this.ask(this.options(origin, types));
    if (result.response !== ALLOW_MEDIA_CHOICE || !this.current(contents, generation)) return false;
    if (!(await this.osConsent(types)) || !this.current(contents, generation)) return false;
    this.remember(contents, types);
    return true;
  }
  /** Keep a single navigation listener per tab, not one per repeated request. */
  private watch(contents: WebContents): number {
    if (!this.generations.has(contents)) {
      this.generations.set(contents, 0);
      contents.on('did-start-navigation', () => this.revoke(contents));
    }
    return this.generations.get(contents)!;
  }
  /** Forget permissions on navigation, including same-URL reloads. */
  private revoke(contents: WebContents): void {
    this.generations.set(contents, this.generations.get(contents)! + 1);
    this.grants.delete(contents);
  }
  /** Prompts cannot grant a replacement document or a browser now controlled by an agent. */
  private current(contents: WebContents, generation: number): boolean {
    return this.allowed(contents) && this.generations.get(contents) === generation;
  }
  /** Separate microphone and camera approvals accumulate only for this document. */
  private remember(contents: WebContents, types: MediaKind[]): void {
    const granted = this.grants.get(contents)?.types || [];
    this.grants.set(contents, { url: contents.getURL(), types: new Set([...granted, ...types]) });
  }
  /** Tell the person exactly which devices the website requested. */
  private options(origin: string, types: MediaKind[]): MessageBoxOptions {
    const devices = types.map((type) => (type === 'audio' ? 'microphone' : 'camera')).join(' and ');
    return {
      message: `Allow ${devices}?`,
      detail: `${origin} wants to use your ${devices}. Allow only if you trust this site. This approval lasts for this page.`,
      buttons: ['Block', 'Allow'],
      defaultId: 0,
      cancelId: 0,
    };
  }
  /** macOS also requires system-level permission; other platforms enforce it themselves. */
  private async osConsent(types: MediaKind[]): Promise<boolean> {
    if (this.platform !== 'darwin') return true;
    for (const type of types) {
      const device = type === 'audio' ? 'microphone' : 'camera';
      if (!(await this.deps.electron.systemPreferences.askForMediaAccess(device))) return this.osDenied();
    }
    return true;
  }
  /** Explain OS denial rather than leaving a call silently without a microphone. */
  private async osDenied(): Promise<false> {
    await this.ask({
      message: 'Device access is blocked in macOS',
      detail: 'Allow Oya Browser in System Settings → Privacy & Security → Microphone or Camera, then retry the call.',
      buttons: ['OK'],
    });
    return false;
  }
  /** Parent to Oya when possible; never pass Electron a null window. */
  private ask(options: MessageBoxOptions) {
    const window = this.deps.shell.window;
    return window && !window.isDestroyed()
      ? this.deps.electron.dialog.showMessageBox(window, options)
      : this.deps.electron.dialog.showMessageBox(options);
  }
}
