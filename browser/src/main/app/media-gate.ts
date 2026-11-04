/**
 * Who may ask a page's device question, and the dialog it is asked with. The
 * one place microphone, camera and screen share all go through, so a grant
 * remembered on disk can never reach a tab an agent drives, a governed
 * browser, a background tab or an insecure page: the gate runs before any
 * store is read, and again after every await.
 */
import type { WebContents, MessageBoxOptions, MessageBoxReturnValue } from 'electron';
import type { AppServices } from './services.ts';
import type { MediaDenial } from './media-prompts.ts';
import { MediaApprovalQueue } from './media-queue.ts';
import { mediaOwner, mediaBinding, sameMediaBinding } from './media-owner.ts';
import { MediaRecovery } from './media-recovery.ts';

/** Sensitive access uses native dialogs and the normal control boundary. */
type Deps = Pick<AppServices, 'electron' | 'shell' | 'tabs' | 'agentTab' | 'governance' | 'persona' | 'windows'>;

/** HTTPS origins are displayed rather than private paths and query strings. */
export function mediaOrigin(raw: string): string | null {
  const url = URL.parse(raw);
  return url?.protocol === 'https:' && !url.username && !url.password ? url.origin : null;
}

/** The rules every device question passes, and the one dialog at a time it is asked in. */
export class MediaGate {
  /** Native APIs, active tab and control policy. */
  private readonly deps: Deps;
  /** Navigation generations fence reloads even when the URL does not change. */
  private readonly generations = new WeakMap<WebContents, number>();
  /** Device-specific recovery keeps independent microphone, camera and screen explanations. */
  private readonly recovery: MediaRecovery;
  /** Committed top documents, counted apart: a navigation that never commits (a download, a 204) leaves a call running. */
  private readonly documents = new WeakMap<WebContents, number>();
  /** The committed document Oya handed a camera, microphone or screen to: that document counts as in a call. */
  private readonly live = new WeakMap<WebContents, number>();
  /** One native consent at a time, with bounded waiting for overlapping media requests. */
  private readonly approvals = new MediaApprovalQueue();

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.recovery = new MediaRecovery(deps.electron, (contents, options) => this.ask(options, contents));
  }

  /** Refuses background tabs, a tab an agent drives, and managed runtimes. Per tab: with agents connected, the person still owns the tabs no agent claimed. */
  allowed(contents: WebContents | null): contents is WebContents {
    if (!contents || contents.isDestroyed() || this.deps.governance.configuration) return false;
    const owner = mediaOwner(this.deps, contents);
    return (
      !!owner &&
      this.deps.agentTab.humanMay(owner.tabs.activeTabId) &&
      owner.tabs.getActiveView()?.webContents === contents
    );
  }

  /** The requesting origin, when `contents` may ask and `raw` is the same secure document; null otherwise. */
  eligible(contents: WebContents | null, raw: string | undefined): string | null {
    if (!this.allowed(contents) || !raw) return null;
    const origin = mediaOrigin(raw);
    return origin && origin === mediaOrigin(contents.getURL()) ? origin : null;
  }

  /** Keeps a single navigation listener per tab. Only a new top document revokes: a pushState or an iframe loading keeps it. Answers the document's generation. */
  watch(contents: WebContents): number {
    if (!this.generations.has(contents)) this.track(contents);
    return this.generations.get(contents)!;
  }

  /** Counts navigations as they start (for grants) and top documents as they commit (for calls). */
  private track(contents: WebContents): void {
    this.generations.set(contents, 0);
    this.documents.set(contents, 0);
    contents.on('did-start-navigation', (e) => e.isMainFrame && !e.isSameDocument && this.revoke(contents));
    contents.on('did-navigate', () => this.documents.set(contents, this.documents.get(contents)! + 1));
  }

  /** A new top document, including a same-URL reload. */
  private revoke(contents: WebContents): void {
    this.generations.set(contents, this.generations.get(contents)! + 1);
  }

  /** A prompt cannot grant a replacement document, or a tab an agent took meanwhile. */
  current(contents: WebContents, generation: number): boolean {
    return this.allowed(contents) && this.generations.get(contents) === generation;
  }

  /** Oya gave this document a device. Electron never says when the page stops the track, so it holds until the document goes. Answers true, the grant. */
  markLive(contents: WebContents): true {
    this.watch(contents);
    this.live.set(contents, this.documents.get(contents)!);
    return true;
  }

  /** Whether this tab's current document was given a device and is still running. Not through `allowed()`: an agent taking the tab does not end the stream. */
  inCall(contents: WebContents): boolean {
    if (contents.isDestroyed() || contents.isCrashed()) return false;
    return this.live.has(contents) && this.live.get(contents) === this.documents.get(contents);
  }

  /** Capture before queueing, so waiting cannot authorize a replacement document or profile. */
  guard(contents: WebContents, origin: string): () => boolean {
    const generation = this.watch(contents);
    const before = mediaBinding(this.deps, contents);
    return () =>
      this.current(contents, generation) &&
      this.eligible(contents, origin) === origin &&
      sameMediaBinding(this.deps, contents, before);
  }

  /** Serializes consent while preserving each callback's original requester and independent decision. */
  locked(current: () => boolean, prompt: () => Promise<boolean>, key?: string): Promise<boolean> {
    return this.approvals.run(current, prompt, key);
  }

  /** A denial applies only to duplicates already waiting for this exact document, profile, site and device set. */
  decisionKey(contents: WebContents, origin: string, devices: string[]): string {
    return JSON.stringify([
      contents.id,
      this.watch(contents),
      this.deps.persona.partitionName(),
      origin,
      [...devices].sort(),
    ]);
  }

  /** Whether the person clicked Allow. */
  async allowedBy(options: MessageBoxOptions, choice: number, contents: WebContents): Promise<boolean> {
    return (await this.ask(options, contents)).response === choice;
  }

  /** Explains the OS denial once per document; always answers false. */
  osDenied(contents: WebContents, current: () => boolean, failure: MediaDenial): Promise<false> {
    return this.recovery.show({ contents, current, failure, generation: this.watch(contents) });
  }

  /** Parents to Oya when possible; never passes Electron a null window. */
  ask(options: MessageBoxOptions, contents: WebContents): Promise<MessageBoxReturnValue> {
    const window = mediaOwner(this.deps, contents)?.shell.window;
    return window && !window.isDestroyed()
      ? this.deps.electron.dialog.showMessageBox(window, options)
      : this.deps.electron.dialog.showMessageBox(options);
  }
}
