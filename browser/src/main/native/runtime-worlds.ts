/** Isolated contexts belong to one connection and one exact parent document, never to public world names. */
import { randomUUID } from 'node:crypto';
import { runtimeIdentity } from './runtime-identity.ts';
import { RUNTIME, type RuntimeContext, type RuntimeReply } from './runtime-types.ts';
/** Creation receives the browser-generated key, not an external numeric world ID. */
type Create = (world: string) => Promise<RuntimeReply>;
/** Revocation uses exact native context metadata even if public creation is cancelled. */
type Release = (main: RuntimeContext, world: string, document: string) => Promise<unknown>;
/** Bounded isolated contexts and coalesced named creation, independent of default context discovery. */
export class RuntimeWorlds {
  /** Successfully created worlds indexed by public unique identity. */
  private readonly contexts = new Map<string, RuntimeContext>();
  /** Concurrent creation of a named world shares one native allocation. */
  private readonly pending = new Map<string, Promise<RuntimeContext>>();
  /** A late engine response cannot repopulate disposed state. */
  private closed = false;
  /** Native cleanup remains available after public ownership has been revoked. */
  private readonly release: Release;
  /** The browser owns cleanup; a page-visible helper cannot substitute for it. */
  constructor(release: Release) {
    this.release = release;
  }
  /** Reuse names only inside this exact live main-world document. */
  create(main: RuntimeContext, name: string, create: Create): Promise<RuntimeContext> {
    if (this.closed) return Promise.reject(Error('Native runtime owner is closed'));
    const known = [...this.contexts.values()].find((c) => c.parent === main.uniqueId && c.name === name && name);
    if (known) return Promise.resolve(known);
    const key = main.uniqueId + ':' + (name || randomUUID());
    if (this.pending.has(key)) return this.pending.get(key)!;
    return this.begin(key, main, name, create);
  }
  /** Reserve capacity before the asynchronous native allocation starts. */
  private begin(key: string, main: RuntimeContext, name: string, create: Create): Promise<RuntimeContext> {
    if (this.contexts.size + this.pending.size >= RUNTIME.worlds)
      return Promise.reject(Error('Native isolated world limit exceeded'));
    const result = this.allocate(main, name, create).finally(() => this.pending.delete(key));
    this.pending.set(key, result);
    return result;
  }
  /** Only engine-issued context tokens can become public contexts. */
  private async allocate(main: RuntimeContext, name: string, create: Create): Promise<RuntimeContext> {
    const world = randomUUID(),
      reply = await create(world);
    if (!reply.context || reply.context === main.document) throw Error('Engine lacks isolated runtime contexts');
    if (this.closed) return this.rejectClosed(main, world, reply.context);
    const context = runtimeIdentity({ ...main, ...worldMetadata(main, world, name), document: reply.context });
    this.contexts.set(context.uniqueId, context);
    return context;
  }
  /** A disconnect in the promise-continuation gap must still revoke the newly created native world. */
  private async rejectClosed(main: RuntimeContext, world: string, document: string): Promise<never> {
    await this.release(main, world, document).catch(() => {});
    throw Error('Native runtime owner is closed');
  }
  /** Failed publication cannot strand a native world or consume public context quota. */
  async discard(context: RuntimeContext): Promise<void> {
    this.contexts.delete(context.uniqueId);
    await this.release(context, context.world!, context.document).catch(() => {});
  }
  /** Return only worlds whose exact parent document was observed live in this tab. */
  live(main: RuntimeContext[]): RuntimeContext[] {
    const parents = new Set(main.map((c) => c.uniqueId));
    return this.all().filter((c) => parents.has(c.parent!));
  }
  /** New native document identities invalidate all previous isolated worlds in that frame. */
  prune(main: RuntimeContext): void {
    for (const [id, context] of this.contexts)
      if (context.frame.detached || (context.frame === main.frame && context.parent !== main.uniqueId))
        this.contexts.delete(id);
  }
  /** Enumerate only this owner's retained world contexts. */
  all(): RuntimeContext[] {
    return [...this.contexts.values()];
  }
  /** Disposal permanently prevents pending responses from restoring connection state. */
  dispose(): void {
    this.closed = true;
    this.contexts.clear();
  }
}

/** Parent identity and main document tokens remain distinct from the isolated context token. */
function worldMetadata(main: RuntimeContext, world: string, name: string) {
  return { world, name, parent: main.uniqueId, mainDocument: main.document };
}
