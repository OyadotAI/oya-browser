/** Revoke one connection's session tree and owned observers without leaving stale CDP routes. */
import type { NativeBackend } from './types.ts';
import type { NativeSessions, NativeSession } from './sessions.ts';
import type { BrowserScopes } from './browser-scopes.ts';
import type { NativeSubscriptions } from './subscriptions.ts';
import type { PassthroughSessions } from './passthrough.ts';

/** Existing protocol collaborators share one resource owner and one backend authority. */
export interface SessionLifecycleDeps {
  /** Exact native target authority shared by browser attachments. */
  readonly backend: NativeBackend;
  /** Bounded, connection-owned attachment hierarchy. */
  readonly sessions: NativeSessions;
  /** Per-browser discovery and automatic attachment observers. */
  readonly scopes: BrowserScopes;
  /** Native event subscriptions held by page sessions. */
  readonly subscriptions: NativeSubscriptions;
  /** Approved page-session event forwarding. */
  readonly through: PassthroughSessions;
  /** Authenticated transport event delivery. */
  readonly emit: (event: object) => void;
}

/** Attachment cleanup remains correct during target revocation, nested detach and disconnect. */
export class SessionLifecycle {
  /** Same collaborators as the owning protocol, never an additional backend connection. */
  private readonly deps: SessionLifecycleDeps;
  /** Manual attachments are revoked even when discovery was never enabled. */
  private stopTargets?: () => void;
  /** A disposed connection cannot acquire any new sessions. */
  private closed = false;

  /** Retain the shared ownership boundary without installing an observer yet. */
  constructor(deps: SessionLifecycleDeps) {
    this.deps = deps;
  }
  /** Dispatch cannot resurrect a closed authenticated connection. */
  requireOpen(): void {
    if (this.closed) throw Error('Native connection is closed');
  }
  /** Watch native authority changes only while pages are attached. */
  watch(): void {
    this.stopTargets ??= this.deps.backend.watchTargets?.(() => this.revokeTargets());
  }
  /** Failed observer setup cannot consume a session or leave a half-attached route. */
  attached(id: string): string {
    try {
      this.watch();
      this.deps.scopes.refresh();
      return id;
    } catch (error) {
      this.close(id);
      throw error;
    }
  }
  /** Descendants are revoked and announced before the browser parent they address. */
  close(id: string): void {
    for (const session of this.deps.sessions.tree(id)) this.release(session);
    this.deps.scopes.refresh();
    if (!this.deps.sessions.pages().length) this.stopWatching();
  }
  /** Revocation reaches every page attachment, including independent browser-root scopes. */
  removed(target: string): void {
    for (const session of this.deps.sessions.pages())
      if (session.target === target && this.deps.sessions.has(session.id)) this.close(session.id);
  }
  /** Read the native target authority again; cached routes never grant continuing ownership. */
  private revokeTargets(): void {
    const live = new Set(this.deps.backend.targets().map((target) => target.targetId));
    for (const session of this.deps.sessions.pages()) if (!live.has(session.target!)) this.removed(session.target!);
  }
  /** Revoke first so reentrant observer disposal cannot announce the same detach twice. */
  private release(session: NativeSession): void {
    const { sessions, subscriptions, through, scopes } = this.deps;
    if (!sessions.has(session.id)) return;
    sessions.delete(session.id);
    subscriptions.release(session.id);
    through.release(session.id);
    scopes.forget(session);
    scopes.release(session.id);
    this.detached(session);
  }
  /** Page events use page sessions; browser management events use their owning browser parent. */
  private detached(session: NativeSession): void {
    const params = { sessionId: session.id, ...(session.target ? { targetId: session.target } : {}) };
    this.deps.emit({
      method: 'Target.detachedFromTarget',
      params,
      ...(session.parent ? { sessionId: session.parent } : {}),
    });
  }
  /** Release the authority observer when no page route can still need it. */
  private stopWatching(): void {
    this.stopTargets?.();
    this.stopTargets = undefined;
  }
  /** Disconnect cleanup is idempotent and disposes the shared backend exactly once. */
  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    this.stopWatching();
    this.deps.scopes.dispose();
    this.deps.subscriptions.dispose();
    this.deps.through.dispose();
    this.deps.sessions.clear();
    this.deps.backend.dispose?.();
  }
}
