/** Browser attachments own independent discovery and auto-attachment state on one shared backend. */
import type { NativeAutoAttach } from './auto-attach.ts';
import type { NativeDiscovery } from './discovery.ts';
import type { NativeSession } from './sessions.ts';

/** Observers and automatic attachments belong to the exact requesting browser session. */
export interface BrowserScope {
  /** Native target discovery for this scope. */
  readonly discovery: NativeDiscovery;
  /** Only automatic page sessions owned by this scope. */
  readonly automatic: NativeAutoAttach;
}

/** Lazily allocate observer collaborators without opening another backend connection. */
export class BrowserScopes {
  /** Scope-local observers keyed by the browser session; undefined is the primary endpoint. */
  private readonly scopes = new Map<string | undefined, BrowserScope>();
  /** Composition callback reuses the owning protocol's backend, registry and event sink. */
  private readonly create: (parent?: string) => BrowserScope;

  /** Retain the composition seam without allocating any observer yet. */
  constructor(create: (parent?: string) => BrowserScope) {
    this.create = create;
  }
  /** Create an observer pair only for the browser scope already authorized by the caller. */
  at(parent?: string): BrowserScope {
    if (!this.scopes.has(parent)) this.scopes.set(parent, this.create(parent));
    return this.scopes.get(parent)!;
  }
  /** Explicit detach relinquishes only the owning browser scope's automatic attachment record. */
  forget(session: NativeSession): void {
    this.scopes.get(session.parent)?.automatic.forget(session.id);
  }
  /** Remove the scope before disposing observers, since disposal can reenter session cleanup. */
  release(id: string | undefined): void {
    const scope = this.scopes.get(id);
    this.scopes.delete(id);
    scope?.discovery.dispose();
    scope?.automatic.dispose();
  }
  /** Attachment changes refresh each enabled scope against its own attachment state. */
  refresh(): void {
    for (const scope of this.scopes.values()) scope.discovery.refresh();
  }
  /** Disconnect removes all scope-local listeners and automatic attachment ownership. */
  dispose(): void {
    for (const id of [...this.scopes.keys()]) this.release(id);
  }
}
