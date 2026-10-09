/** Wait for a staged Google app's main document, not slow subframes or third-party resources. */
import type { Tab } from './types.ts';
import { GOOGLE_APP_HANDOFF_TIMEOUT } from './constants.ts';

/** A document-ready signal must belong to the expected protected app, never its initial blank page. */
type Accept = () => boolean;

/** Owns all listeners and the deadline for one background handoff. */
export class GoogleAppLoad {
  /** The staged page whose main document must be ready. */
  private readonly tab: Tab;
  /** Rechecks the destination and protection at each readiness signal. */
  private readonly accept: Accept;
  /** Settles once, including when the original load promise later fails. */
  private done = false;
  /** Stops a slow or abandoned load from retaining listeners forever. */
  private timer?: NodeJS.Timeout;
  /** Resolves a confirmed main-document load. */
  private resolve: () => void = () => {};
  /** Rejects a failed or abandoned load. */
  private reject: (error: Error) => void = () => {};
  /** Bound listeners can be removed on every terminal path. */
  private readonly loaded = (): void => {
    if (this.accept()) this.finish();
  };
  /** A destroyed candidate cannot replace a verified popup. */
  private readonly destroyed = (): void => this.finish(new Error('Sign-in destination closed'));

  /** A completed load at an unexpected address is not a successful sign-in handoff. */
  private readonly completed = (): void => {
    this.finish(this.accept() ? undefined : new Error('Sign-in destination changed'));
  };
  /** A failed navigation must preserve the verified original popup. */
  private readonly failed = (): void => this.finish(new Error('Sign-in destination failed'));

  /** The caller supplies the approved destination check; this class only owns readiness. */
  constructor(tab: Tab, accept: Accept) {
    this.tab = tab;
    this.accept = accept;
  }

  /** Observe DOM readiness before waiting for loadURL, which includes frames unrelated to sign-in. */
  wait(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.resolve = resolve;
      this.reject = reject;
      this.install();
      this.tab.ready?.then(this.completed, this.failed);
    });
  }

  /** Ignore blank-page events through accept(), and bound the candidate's lifetime. */
  private install(): void {
    const contents = this.tab.view.webContents;
    contents.on('dom-ready', this.loaded);
    contents.on('did-finish-load', this.loaded);
    contents.on('destroyed', this.destroyed);
    this.timer = setTimeout(() => this.finish(new Error('Sign-in destination timed out')), GOOGLE_APP_HANDOFF_TIMEOUT);
  }

  /** Remove listeners before notifying the caller, so closing the candidate cannot race settlement. */
  private finish(error?: Error): void {
    if (this.done) return;
    this.done = true;
    clearTimeout(this.timer);
    this.cleanup();
    if (error) this.reject(error);
    else this.resolve();
  }

  /** Release all page listeners on success, failure, timeout or destruction. */
  private cleanup(): void {
    const contents = this.tab.view.webContents;
    contents.removeListener('dom-ready', this.loaded);
    contents.removeListener('did-finish-load', this.loaded);
    contents.removeListener('destroyed', this.destroyed);
  }
}
