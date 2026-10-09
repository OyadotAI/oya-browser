/** Observe actual document readiness and frame removal without polling or debugger subscriptions. */
import type { WebContents, WebFrameMain } from 'electron';
import { NATIVE_RECORDING } from '../../shared/native-recording.ts';
/** Browser-owned handlers for one recording's frame lifecycle. */
export interface RecordingFrameEvents {
  /** Current document is ready for authorization. */
  ready(frame: WebFrameMain): void;
  /** Frame left the owned native graph. */
  removed(frame: WebFrameMain): void;
  /** Native observation failed and must be reported. */
  failed(error: unknown): void;
}
/** One listener belongs to each native frame wrapper, independent of its changing URL. */
export class RecordingFrames {
  /** The exact page this recording owns. */
  private readonly page: WebContents;
  /** The recording channel consumes native lifecycle events. */
  private readonly events: RecordingFrameEvents;
  /** Repeated starts cannot duplicate native listeners. */
  private started = false;
  /** Stable listener identities allow exact teardown. */
  private readonly frames = new Map<WebFrameMain, () => void>();
  /** Disposal fences events already queued by the engine. */
  private closed = false;
  /** Composition supplies behavior; this observer never executes page scripts. */
  constructor(page: WebContents, events: RecordingFrameEvents) {
    this.page = page;
    this.events = events;
  }
  /** Subscribe before inspecting the initial graph so a concurrent child cannot be missed. */
  start(): WebFrameMain[] {
    if (this.closed) throw Error('Recording frame observer is stopped');
    requireRecordingFrames(this.page);
    if (!this.started) this.listen();
    this.initialGraph();
    return [...this.frames.keys()];
  }
  /** Initial failure releases partial subscriptions before propagating to the recording start. */
  private initialGraph(): void {
    try {
      this.synchronize();
    } catch (error) {
      this.stop();
      throw error;
    }
  }
  /** These subscriptions are installed once before initial enumeration. */
  private listen(): void {
    this.started = true;
    this.page.on('frame-created', this.created);
    this.page.on('oya-frame-tree-changed' as 'dom-ready', this.changed);
    this.page.once('destroyed', this.stop);
  }
  /** Exact native graph membership, not URL matching, determines which wrappers are live. */
  private synchronize(): void {
    if (this.closed || this.page.isDestroyed()) return;
    const live = new Set(this.page.mainFrame.framesInSubtree);
    if (live.size > NATIVE_RECORDING.MAX_DOCUMENTS) throw Error('Native recording frame limit exceeded');
    for (const [frame, listener] of this.frames) if (!live.has(frame) || frame.detached) this.drop(frame, listener);
    for (const frame of live) this.watch(frame);
  }
  /** New document readiness is reported directly by the native frame, including cross-process children. */
  private watch(frame: WebFrameMain): void {
    if (frame.detached || this.frames.has(frame)) return;
    const listener = (): void => {
      if (!this.closed && !frame.detached) this.events.ready(frame);
    };
    this.frames.set(frame, listener);
    frame.on('dom-ready', listener);
  }
  /** A new child is subscribed before its first document becomes ready. */
  private readonly created = (_event: unknown, details: CreatedRecordingFrame): void => {
    if (this.closed) return;
    try {
      if (details.frame) this.watch(details.frame);
      this.synchronize();
    } catch (error) {
      this.fail(error);
    }
  };
  /** Stop observing before reporting so repeated invalid graphs cannot accumulate listeners. */
  private fail(error: unknown): void {
    this.stop();
    this.events.failed(error);
  }
  /** Removal signals prune controllers without deleting authorization for final unload IPC. */
  private readonly changed = (): void => {
    if (this.closed) return;
    try {
      this.synchronize();
    } catch (error) {
      this.fail(error);
    }
  };
  /** Removing one frame never cancels a sibling frame's listener or pending recorder. */
  private drop(frame: WebFrameMain, listener: () => void): void {
    frame.off('dom-ready', listener);
    this.frames.delete(frame);
    this.events.removed(frame);
  }
  /** Remove only this recording's listeners; a late engine event cannot restart it. */
  readonly stop = (): void => {
    if (this.closed) return;
    this.closed = true;
    this.page.off('frame-created', this.created);
    this.page.off('oya-frame-tree-changed' as 'dom-ready', this.changed);
    this.page.off('destroyed', this.stop);
    for (const [frame, listener] of this.frames) frame.off('dom-ready', listener);
    this.frames.clear();
  };
}
/** Older engines cannot approximate native removal notifications using a debugger or a timer. */
function requireRecordingFrames(page: WebContents): void {
  const native = page as WebContents & {
    /** Explicit native lifecycle capability. */ _supportsOyaFrameLifecycle?: () => boolean;
  };
  if (page.isDestroyed()) throw Error('View is destroyed');
  if (!native._supportsOyaFrameLifecycle?.()) throw Error('Oya engine lacks native recording frame lifecycle');
}

/** Native frame-created event carries a revocable browser-owned wrapper. */
interface CreatedRecordingFrame {
  /** Newly created frame, if it is still alive. */
  frame: WebFrameMain | null;
}
