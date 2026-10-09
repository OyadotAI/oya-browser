/** Native document contexts and connection-owned value cleanup, without page-visible registries. */
import { randomUUID } from 'node:crypto';
import type { NativePage } from './page.ts';
import { RUNTIME, type RuntimeFrame, type RuntimeContext, type RuntimeReply } from './runtime-types.ts';
/** Context IDs cannot alias a different socket or replacement document. */
let sequence = 0;
/** One owner namespace is private to one external native connection. */
export class RuntimeContexts {
  /** Opaque value namespace never supplied by the page. */
  readonly owner = randomUUID();
  /** Exact frame wrappers retained solely for context validation and disposal. */
  private readonly frames = new Map<RuntimeFrame, RuntimeContext>();
  /** Disconnect permanently revokes the owner, including late context replies. */
  private closed = false;
  /** Child-frame identities agree with Page.getFrameTree in the composition root. */
  constructor(frameId: (frame: RuntimeFrame) => string = () => randomUUID()) {
    this.frameId = frameId;
  }
  /** Connection-local frame identity provider. */
  private readonly frameId: (frame: RuntimeFrame) => string;
  /** Read native document identity before every operation rather than trusting cached URLs. */
  async current(page: NativePage, target: string, selected?: RuntimeFrame): Promise<RuntimeContext> {
    if (this.closed || page.webContents.isDestroyed()) throw Error('Native runtime owner is closed');
    const frame = selected || (page.webContents.mainFrame as RuntimeFrame);
    this.requireOwned(page, frame);
    const reply = await this.call(frame, '', 'context', {});
    this.requireOwned(page, frame);
    if (this.closed) throw Error('Native runtime owner is closed');
    if (!reply.context) throw Error('Native runtime returned no context identity');
    return this.rememberFrame(frame, target, reply.context, page);
  }
  /** Root identity equals the protected target; descendants share the native frame-tree registry. */
  private rememberFrame(frame: RuntimeFrame, target: string, document: string, page: NativePage): RuntimeContext {
    const id = frame === page.webContents.mainFrame ? target : this.frameId(frame);
    return this.remember(frame, target, document, id);
  }

  /** Retain a new public identity only when the native document actually changes. */
  private remember(frame: RuntimeFrame, target: string, document: string, frameId: string): RuntimeContext {
    for (const known of this.frames.keys()) if (known.detached) this.frames.delete(known);
    const previous = this.frames.get(frame);
    if (previous?.document === document) return previous;
    if (!previous && this.frames.size >= RUNTIME.frames) throw Error('Native runtime frame limit exceeded');
    if (!Number.isSafeInteger(++sequence)) throw Error('Native context identity space exhausted');
    const context = { id: sequence, uniqueId: randomUUID(), document, frame, target, frameId };
    this.frames.set(frame, context);
    return context;
  }
  /** Membership in the exact native frame graph is required before and after every asynchronous read. */
  requireOwned(page: NativePage, frame: RuntimeFrame): void {
    if (page.webContents.isDestroyed() || frame.detached) throw Error('Native runtime frame is closed');
    const top = page.webContents.mainFrame;
    if (frame !== top && !top.framesInSubtree.includes(frame)) throw Error('Stale or foreign native frame');
  }
  /** Enumerate only native main-world contexts in this exact live tab. */
  async all(page: NativePage, target: string): Promise<RuntimeContext[]> {
    const frames = page.webContents.mainFrame.framesInSubtree;
    if (frames.length > RUNTIME.frames) throw Error('Native runtime frame limit exceeded');
    const results = await Promise.allSettled(frames.map((frame) => this.current(page, target, frame)));
    return results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []));
  }
  /** Select previously issued context identities, never arbitrary renderer process IDs. */
  selected(target: string, params: Record<string, unknown>): RuntimeContext | undefined {
    const id = params.contextId ?? params.executionContextId,
      unique = params.uniqueContextId;
    if (id === undefined && unique === undefined) return;
    const context = [...this.frames.values()].find(
      (c) => c.target === target && (id !== undefined ? c.id === id : c.uniqueId === unique),
    );
    if (!context) throw Error('Stale or foreign native execution context');
    return context;
  }
  /** Known documents on one exact target are the only scope of group release. */
  forTarget(target: string): RuntimeContext[] {
    return [...this.frames.values()].filter((context) => context.target === target && !context.frame.detached);
  }
  /** Reject foreign or stale public context identities before engine execution. */
  validate(context: RuntimeContext, params: Record<string, unknown>): void {
    const id = params.contextId ?? params.executionContextId;
    if (id !== undefined && id !== context.id) throw Error('Stale or foreign native execution context');
    if (params.uniqueContextId !== undefined && params.uniqueContextId !== context.uniqueId)
      throw Error('Stale or foreign native execution context');
  }
  /** Only the explicit engine capability can execute a main-world operation. */
  async call(frame: RuntimeFrame, document: string, operation: string, params: object): Promise<RuntimeReply> {
    if (frame.detached || (this.closed && operation !== 'close'))
      throw Error('Native runtime frame or owner is closed');
    if (!frame._runOyaRuntime) throw Error('This Oya engine lacks native main-world runtime operations');
    const reply = await frame._runOyaRuntime(this.owner, document, operation, params);
    if (this.closed && operation !== 'close') throw Error('Native runtime owner is closed');
    if (reply.error) throw Error(reply.error);
    return reply;
  }
  /** Release values in live native documents; late promises cannot re-create a released owner. */
  dispose(): void {
    this.closed = true;
    for (const context of this.frames.values())
      if (!context.frame.detached) void this.call(context.frame, context.document, 'close', {}).catch(() => {});
    this.frames.clear();
  }
}
