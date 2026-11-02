/** Pending layout must finish on the intended native document before a wheel's default scroll target is chosen. */
import type { WebFrameMain } from 'electron';
import type { NativePage } from '../native/index.ts';

/** An isolated native read flushes layout without changing scroll state or returning page content. */
type LayoutRead = (frame: WebFrameMain) => Promise<unknown>;

/** Pin document identity throughout readiness and recheck caller ownership immediately before native input. */
export function withPointerReadiness(
  page: NativePage,
  read: LayoutRead,
  guard: () => void,
  send: () => void,
): Promise<void> {
  const document = new PointerDocument(page);
  return document.run(read, guard, send).finally(() => document.dispose());
}

/** Transient listeners remember replacement even when navigation reuses the same frame object or URL. */
class PointerDocument {
  /** The captured frame is never refreshed to a replacement document. */
  readonly frame: WebFrameMain;
  /** Any document retirement makes this input permanently stale. */
  private stale = false;
  /** The exact already-authorized WebContents remains the event destination. */
  private readonly page: NativePage;
  /** Non-navigation retirement is terminal regardless of the frame wrapper's lifetime. */
  private readonly retired = () => {
    this.stale = true;
  };
  /** Same-document navigation and other frames do not replace the intended main document. */
  private readonly navigating = (_event: unknown, _url: string, sameDocument: boolean, mainFrame: boolean) => {
    if (mainFrame && !sameDocument) this.stale = true;
  };
  /** Observe replacement before starting any asynchronous renderer work. */
  constructor(page: NativePage) {
    this.page = page;
    this.frame = page.webContents.mainFrame;
    this.assertCurrent();
    page.webContents.on('did-start-navigation', this.navigating);
    page.webContents.on('did-navigate', this.retired);
    page.webContents.on('render-process-gone', this.retired);
    page.webContents.on('destroyed', this.retired);
  }
  /** No await may separate final document/owner admission from the real native event. */
  async run(read: LayoutRead, guard: () => void, send: () => void): Promise<void> {
    await read(this.frame);
    this.assertCurrent();
    guard();
    this.assertCurrent();
    send();
  }
  /** The same wrapper alone cannot establish document identity after a navigation. */
  assertCurrent(): void {
    const contents = this.page.webContents;
    if (this.stale || contents.isDestroyed() || this.frame.detached || contents.mainFrame !== this.frame)
      throw Error('Native pointer document changed before input dispatch');
  }
  /** Success, rejection, cancellation and renderer errors release exactly this operation's listeners. */
  dispose(): void {
    const contents = this.page.webContents;
    contents.removeListener('did-start-navigation', this.navigating);
    contents.removeListener('did-navigate', this.retired);
    contents.removeListener('render-process-gone', this.retired);
    contents.removeListener('destroyed', this.retired);
  }
}
