/** Main-world context events are native lifecycle observations, not inspector notifications. */
import type { NativePage } from './page.ts';
import type { NativeEventSink } from './log-stream.ts';
import { RUNTIME, type RuntimeContext, type RuntimeFrame } from './runtime-types.ts';
/** Runtime watchers share only this connection's context lookup and authorization guard. */
export interface RuntimeWatch {
  /** Exact protected page. */
  page: NativePage;
  /** Obtain the real current native main-world context. */
  current(): Promise<RuntimeContext[]>;
  /** Recheck human ownership before every event, including delayed replies. */
  allowed(): boolean;
  /** Authenticated event sink. */
  emit: NativeEventSink;
}
/** Serialize only the native default main context; no preload or agent-world contexts are advertised. */
function describe(context: RuntimeContext): object {
  return {
    id: context.id,
    uniqueId: context.uniqueId,
    origin: context.frame.origin,
    name: '',
    auxData: { isDefault: true, type: 'default', frameId: context.frameId },
  };
}
/** Each enable owns its watcher; closing it invalidates asynchronous context reads. */
export class RuntimeEvents {
  /** Dependencies retain exact-page isolation and control ownership. */
  private readonly deps: RuntimeWatch;
  /** Latest context sent to this subscriber, not a global cache. */
  private readonly known = new Map<string, RuntimeContext>();
  /** Native frame readiness listeners belong only to this observer. */
  private readonly frames = new Set<RuntimeFrame>();
  /** A disconnect prevents late replies from emitting anything. */
  private closed = false;
  /** Increasing read epoch prevents out-of-order lifecycle replies. */
  private epoch = 0;
  /** Store dependencies without starting asynchronous observation implicitly. */
  constructor(deps: RuntimeWatch) {
    this.deps = deps;
  }
  /** Initial context is obtained synchronously by Runtime.enable's native preflight. */
  start(): () => void {
    const wc = this.deps.page.webContents;
    requireFrameLifecycle(this.deps.page);
    this.listenFrames();
    wc.on('did-navigate', this.clear);
    wc.on('render-process-gone', this.clear);
    wc.on('destroyed', this.clear);
    this.refresh();
    return () => this.stop();
  }
  /** Read native context identity at readiness; never replay human-held page state. */
  private readonly refresh = (): void => {
    const epoch = ++this.epoch;
    try {
      this.watchFrames();
    } catch {
      return this.failed(epoch);
    }
    if (this.closed || !this.deps.allowed()) return;
    this.read(epoch);
  };
  /** Observe actual native graph and navigation signals, never a polling timer. */
  private listenFrames(): void {
    const wc = this.deps.page.webContents;
    wc.on('dom-ready', this.refresh);
    wc.on('did-frame-navigate', this.refresh);
    wc.on('frame-created', this.created);
    wc.on('oya-frame-tree-changed' as 'dom-ready', this.refresh);
  }
  /** Every asynchronous enumeration is bounded by the current lifecycle epoch. */
  private read(epoch: number): void {
    void this.deps
      .current()
      .then((contexts) => this.deliver(contexts, epoch))
      .catch(() => this.failed(epoch));
  }

  /** Stale reads and unauthorized activity cannot announce contexts to an agent. */
  private deliver(contexts: RuntimeContext[], epoch: number): void {
    if (this.closed || epoch !== this.epoch || !this.deps.allowed()) return;
    const live = new Map(contexts.filter((c) => !c.frame.detached).map((c) => [c.uniqueId, c]));
    for (const [id, context] of this.known) if (!live.has(id)) this.destroyed(context);
    for (const [id, context] of live) if (!this.known.has(id)) this.announce(context);
  }
  /** Child replacement destroys only that document, not surviving sibling contexts. */
  private destroyed(context: RuntimeContext): void {
    this.known.delete(context.uniqueId);
    this.deps.emit('Runtime.executionContextDestroyed', {
      executionContextId: context.id,
      executionContextUniqueId: context.uniqueId,
    });
  }
  /** A native context is announced at most once per subscription. */
  private announce(context: RuntimeContext): void {
    this.known.set(context.uniqueId, context);
    this.deps.emit('Runtime.executionContextCreated', { context: describe(context) });
  }
  /** Ready events are native per-frame events, including cross-process children. */
  private watchFrames(): void {
    for (const frame of this.frames)
      if (frame.detached) {
        frame.off('dom-ready', this.refresh);
        this.frames.delete(frame);
      }
    if (this.closed || this.deps.page.webContents.isDestroyed()) return;
    for (const frame of this.deps.page.webContents.mainFrame.framesInSubtree) this.watchFrame(frame);
  }
  /** Subscribe before a newly attached child's first document is ready. */
  private readonly created = (
    _event: unknown,
    details: { /** Newly created native wrapper, possibly revoked before delivery. */ frame: RuntimeFrame | null },
  ): void => {
    try {
      if (!this.closed && details.frame) this.watchFrame(details.frame);
    } catch {
      this.clear();
    }
  };
  /** Native graph membership is validated by context discovery, not by the event payload. */
  private watchFrame(frame: RuntimeFrame): void {
    if (frame.detached || this.frames.has(frame)) return;
    if (this.frames.size >= RUNTIME.frames) return;
    this.frames.add(frame);
    frame.on('dom-ready', this.refresh);
  }
  /** A failed refresh invalidates a previously delivered context rather than leaving it advertised. */
  private failed(epoch: number): void {
    if (epoch === this.epoch) this.clear();
  }
  /** Renderer loss clears previously delivered state only while this observer is authorized. */
  private readonly clear = (): void => {
    this.epoch++;
    if (!this.closed && this.known.size && this.deps.allowed()) this.deps.emit('Runtime.executionContextsCleared', {});
    this.known.clear();
  };
  /** Remove only this connection's native listeners; all pending reads become inert. */
  private stop(): void {
    this.closed = true;
    this.epoch++;
    const wc = this.deps.page.webContents;
    this.unwatchFrames();
    wc.off('dom-ready', this.refresh);
    wc.off('did-navigate', this.clear);
    wc.off('render-process-gone', this.clear);
    wc.off('destroyed', this.clear);
  }
  /** Dispose every child listener and prevent future native creation/removal signals. */
  private unwatchFrames(): void {
    const wc = this.deps.page.webContents;
    wc.off('did-frame-navigate', this.refresh);
    wc.off('frame-created', this.created);
    wc.off('oya-frame-tree-changed' as 'dom-ready', this.refresh);
    for (const frame of this.frames) frame.off('dom-ready', this.refresh);
    this.frames.clear();
    this.known.clear();
  }
}

/** Older engines must not advertise child contexts without native removal notifications. */
function requireFrameLifecycle(page: NativePage): void {
  const wc = page.webContents as typeof page.webContents & {
    /** Native capability marker. */ _supportsOyaFrameLifecycle?: () => boolean;
  };
  if (!wc._supportsOyaFrameLifecycle?.()) throw Error('This Oya engine lacks native frame lifecycle notifications');
}
