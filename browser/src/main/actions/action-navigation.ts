/** Observe navigation caused during an input action, never unrelated pre-existing page resource loads. */
import type { WebContents } from 'electron';
import { LOAD_TIMEOUT_MS, NAVIGATION_START_MS } from './constants.ts';
import { sleep } from '../input/timing.ts';
/** Exact native input surface; no active-tab lookup or debugging transport. */
type Page = { /** Command-owned native contents. */ webContents: WebContents };
/** Install before dispatch so a fast native navigation cannot be missed. */
export async function actionNavigation(page: Page, action: () => Promise<void>): Promise<boolean> {
  const watch = new ActionNavigation(page.webContents);
  try {
    await action();
    return await watch.settled();
  } finally {
    watch.dispose();
  }
}
/** A command owns its listeners and deadline; another caller's event subscriptions remain intact. */
class ActionNavigation {
  /** Native event emitter and destruction state. */
  private readonly contents: WebContents;
  /** Only a new main-document navigation starts a wait. */
  private started = false;
  /** Native main-document readiness, independent of advertisements/subresources. */
  private ready = false;
  /** Native destruction or main-frame failure is explicit, not a successful load. */
  private failure = '';
  /** Wake the optional waiter exactly when terminal native state changes. */
  private wake?: () => void;
  /** Bound only an actual newly started navigation. */
  private timer?: ReturnType<typeof setTimeout>;
  /** Exact listeners are retained for deterministic cleanup on every exit path. */
  private readonly listeners: Record<string, (...args: unknown[]) => void> = {
    'did-start-navigation': (details) => this.start(details),
    'dom-ready': () => this.finish(),
    'did-fail-load': (_event, _code, _text, _url, main) => {
      if (this.started && main === true) this.fail('Main-frame navigation failed');
    },
    destroyed: () => this.fail('Navigation target was destroyed'),
    'render-process-gone': () => this.fail('Navigation renderer was lost'),
  };
  /** Register synchronously before the click or Enter reaches the renderer. */
  constructor(contents: WebContents) {
    this.contents = contents;
    for (const [event, listener] of Object.entries(this.listeners))
      (contents as NodeJS.EventEmitter).on(event, listener);
  }
  /** Same-document and child navigations must not wait for an unrelated top-level load. */
  private start(details: unknown): void {
    const event = details as {
      /** Excludes child documents. */ isMainFrame?: boolean;
      /** Excludes fragment/history changes. */ isSameDocument?: boolean;
    } | null;
    if (event?.isMainFrame !== true || event.isSameDocument === true) return;
    this.started = true;
    this.ready = false;
  }
  /** A pre-existing document's readiness is irrelevant until a new navigation starts. */
  private finish(): void {
    if (!this.started) return;
    this.ready = true;
    this.wake?.();
  }
  /** A renderer loss is fatal even when input never starts navigation. */
  private fail(message: string): void {
    this.failure = message;
    this.wake?.();
  }
  /** Keep the existing brief navigation-start grace, but never wait on isLoading(). */
  async settled(): Promise<boolean> {
    await sleep(NAVIGATION_START_MS);
    if (this.started && !this.ready && !this.failure) await this.wait();
    if (this.failure || this.contents.isDestroyed()) throw Error(this.failure || 'Navigation target was destroyed');
    if (this.started && !this.ready) throw Error('Main-frame navigation readiness timed out');
    return this.started;
  }
  /** The deadline is canceled even when the action itself throws or the renderer disappears. */
  private wait(): Promise<void> {
    return new Promise((resolve) => {
      this.wake = resolve;
      this.timer = setTimeout(resolve, LOAD_TIMEOUT_MS);
    });
  }
  /** Remove only this input action's listeners and timer. */
  dispose(): void {
    if (this.timer !== undefined) clearTimeout(this.timer);
    for (const [event, listener] of Object.entries(this.listeners))
      (this.contents as NodeJS.EventEmitter).off(event, listener);
  }
}
