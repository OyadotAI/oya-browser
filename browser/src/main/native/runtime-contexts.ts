/** Native document contexts and connection-owned value cleanup, without page-visible registries. */
import { randomUUID } from 'node:crypto';
import type { NativePage } from './page.ts';
import { RUNTIME, type RuntimeFrame, type RuntimeContext, type RuntimeReply } from './runtime-types.ts';
import { runtimeIdentity } from './runtime-identity.ts';
import { RuntimeWorlds } from './runtime-worlds.ts';
/** Correlate a native response with its exact frame and request metadata. */
type RuntimeCallReply = [frame: RuntimeFrame, operation: string, params: object, reply: RuntimeReply];
/** One owner namespace is private to one external native connection. */
export class RuntimeContexts {
  /** Opaque value namespace never supplied by the page. */
  readonly owner = randomUUID();
  /** Exact frame wrappers retained solely for context validation and disposal. */
  private readonly frames = new Map<RuntimeFrame, RuntimeContext>();
  /** Disconnect permanently revokes the owner, including late context replies. */
  private closed = false;
  /** Agent-created worlds never share the internal analyzer or recorder world. */
  private readonly worlds = new RuntimeWorlds((main, world, document) =>
    this.call(main.frame, document, 'close', { world }),
  );
  /** Context creation refreshes active observers without polling. */
  private readonly listeners = new Set<() => void>();
  /** Native lifecycle observers subscribe only while Runtime is enabled. */
  subscribe(changed: () => void): () => void {
    this.listeners.add(changed);
    return () => {
      this.listeners.delete(changed);
    };
  }
  /** Resolve frame IDs through the live native graph, never process/routing identifiers. */
  async create(page: NativePage, target: string, params: Record<string, unknown>): Promise<RuntimeContext> {
    const frame = this.findFrame(page, target, params.frameId);
    const main = await this.current(page, target, frame);
    const context = await this.worlds.create(main, String(params.worldName ?? ''), (world) =>
      this.call(frame, main.document, 'isolatedContext', { world }),
    );
    await this.publish(page, target, context);
    return this.announce(context);
  }
  /** Disconnect between allocation and publication cannot return a revived public identity. */
  private announce(context: RuntimeContext): RuntimeContext {
    if (this.closed) throw Error('Native runtime owner is closed');
    for (const changed of this.listeners) changed();
    return context;
  }
  /** Check ownership again after allocation and revoke worlds that lost their document while awaiting. */
  private async publish(page: NativePage, target: string, context: RuntimeContext): Promise<void> {
    try {
      await this.resolve(page, target, context);
    } catch (error) {
      await this.worlds.discard(context);
      throw error;
    }
  }
  /** A frame name or numeric process ID can never substitute for an issued frame identity. */
  private findFrame(page: NativePage, target: string, id: unknown): RuntimeFrame {
    const frames = page.webContents.mainFrame.framesInSubtree;
    if (frames.length > RUNTIME.frames) throw Error('Native runtime frame limit exceeded');
    const frame = frames.find((f) => (f === page.webContents.mainFrame ? target : this.frameId(f)) === id);
    if (!frame) throw Error('Stale or foreign native frame');
    return frame;
  }
  /** Main-document and native world identities must both survive asynchronous selection. */
  async resolve(page: NativePage, target: string, selected?: RuntimeContext): Promise<RuntimeContext> {
    const main = await this.current(page, target, selected?.frame);
    if (!selected?.world) return main;
    if (selected.parent !== main.uniqueId) throw Error('Stale or foreign native execution context');
    const reply = await this.perform(selected, 'context', {});
    this.requireOwned(page, selected.frame);
    if (reply.context !== selected.document) throw Error('Stale or foreign native execution context');
    return selected;
  }
  /** Attach world metadata only from the owned context, never external parameters. */
  perform(context: RuntimeContext, operation: string, params: object): Promise<RuntimeReply> {
    return this.call(context.frame, context.document, operation, {
      ...params,
      ...(context.world ? { world: context.world } : {}),
    });
  }
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
    if (previous?.document === document) return this.retain(previous);
    if (!previous && this.frames.size >= RUNTIME.frames) throw Error('Native runtime frame limit exceeded');
    return this.retain(runtimeIdentity({ document, frame, target, frameId }));
  }
  /** Retaining a current main context also invalidates its replaced isolated children. */
  private retain(context: RuntimeContext): RuntimeContext {
    this.worlds.prune(context);
    this.frames.set(context.frame, context);
    return context;
  }
  /** Membership in the exact native frame graph is required before and after every asynchronous read. */
  requireOwned(page: NativePage, frame: RuntimeFrame): void {
    if (page.webContents.isDestroyed() || frame.detached) throw Error('Native runtime frame is closed');
    const top = page.webContents.mainFrame;
    if (frame !== top && !top.framesInSubtree.includes(frame)) throw Error('Stale or foreign native frame');
  }
  /** Enumerate native main contexts and owned isolated worlds in this exact live tab. */
  async all(page: NativePage, target: string): Promise<RuntimeContext[]> {
    const frames = page.webContents.mainFrame.framesInSubtree;
    if (frames.length > RUNTIME.frames) throw Error('Native runtime frame limit exceeded');
    const results = await Promise.allSettled(frames.map((frame) => this.current(page, target, frame)));
    const main = results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []));
    return [...main, ...this.worlds.live(main)];
  }
  /** Select previously issued context identities, never arbitrary renderer process IDs. */
  selected(target: string, params: Record<string, unknown>): RuntimeContext | undefined {
    const id = params.contextId ?? params.executionContextId,
      unique = params.uniqueContextId;
    if (id === undefined && unique === undefined) return;
    const context = [...this.frames.values(), ...this.worlds.all()].find(
      (c) => c.target === target && (id !== undefined ? c.id === id : c.uniqueId === unique),
    );
    if (!context) throw Error('Stale or foreign native execution context');
    return context;
  }
  /** Known documents on one exact target are the only scope of group release. */
  forTarget(target: string): RuntimeContext[] {
    return [...this.frames.values(), ...this.worlds.all()].filter(
      (context) => context.target === target && !context.frame.detached,
    );
  }
  /** Reject foreign or stale public context identities before engine execution. */
  validate(context: RuntimeContext, params: Record<string, unknown>): void {
    const id = params.contextId ?? params.executionContextId;
    if (id !== undefined && id !== context.id) throw Error('Stale or foreign native execution context');
    if (params.uniqueContextId !== undefined && params.uniqueContextId !== context.uniqueId)
      throw Error('Stale or foreign native execution context');
  }
  /** Only the explicit engine capability can execute an owned native runtime operation. */
  async call(frame: RuntimeFrame, document: string, operation: string, params: object): Promise<RuntimeReply> {
    if (frame.detached || (this.closed && operation !== 'close'))
      throw Error('Native runtime frame or owner is closed');
    if (!frame._runOyaRuntime) throw Error('This Oya engine lacks native main-world runtime operations');
    const reply = await frame._runOyaRuntime(this.owner, document, operation, params);
    return this.finishCall(frame, operation, params, reply);
  }
  /** A world created while disconnecting is explicitly revoked before discarding its reply. */
  private async finishCall(...[frame, operation, params, reply]: RuntimeCallReply): Promise<RuntimeReply> {
    if (this.closed && operation === 'isolatedContext' && reply.context)
      await frame._runOyaRuntime!(this.owner, reply.context, 'close', params).catch(() => {});
    if (this.closed && operation !== 'close') throw Error('Native runtime owner is closed');
    if (reply.error) throw Error(reply.error);
    return reply;
  }
  /** Release values in live native documents; late promises cannot re-create a released owner. */
  dispose(): void {
    this.closed = true;
    for (const context of [...this.frames.values(), ...this.worlds.all()])
      if (!context.frame.detached) void this.perform(context, 'close', {}).catch(() => {});
    this.frames.clear();
    this.worlds.dispose();
    this.listeners.clear();
  }
}
