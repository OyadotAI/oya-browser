/**
 * Page sessions whose standard methods pass through to the tab (the backend's
 * optional `passthrough`), with their event streams and the iframe and worker
 * sessions the tab attaches itself. Oya.* methods and every browser-scoped
 * method stay native. Ownership is the protocol's: a page session must map to
 * an authorized target, and a child session only exists under one.
 */
import type { NativeBackend, NativeCommand } from './types.ts';
import { ChildSessions, type TabEvent } from './child-sessions.ts';
import { NATIVE_DOOR } from './constants.ts';

/** Delivers one event to the client, under `sessionId` when it has one. */
type Emit = (event: object) => void;

/** Page watches are revoked before their backend listener is stopped. */
interface Watch {
  /** Late callbacks cannot recreate aliases after release. */
  active: boolean;
  /** Remove only the listener owned by this page attachment. */
  stop: () => void;
}
/** Protocol composition shares capacity and the existing fatal transport-close path. */
interface Limits {
  /** Account for all live child aliases alongside native attachments. */
  capacity: (children: number) => void;
  /** Revoke the transport if asynchronous observer admission fails. */
  fatal: (reason: string) => void;
}

/** One connection's passed-through page and child sessions. */
export class PassthroughSessions {
  /** The app backend, which owns the actual passthrough. */
  private readonly backend: NativeBackend;
  /** Client event delivery. */
  private readonly emit: Emit;
  /** Event forwarding per page session key (its session id, or the pinned target). */
  private readonly watches = new Map<string, Watch>();
  /** Page-local opaque aliases for iframe and nested worker sessions. */
  private readonly children: ChildSessions;
  /** Optional connection composition; standalone users still revoke their own observers. */
  private readonly limits?: Limits;

  /** Keeps the backend and the client's event sink. */
  constructor(backend: NativeBackend, emit: Emit, limits?: Limits) {
    this.backend = backend;
    this.emit = emit;
    this.limits = limits;
    this.children = new ChildSessions(limits?.capacity ?? childCapacity);
  }
  /** Native allocations include child aliases in their existing admission budget. */
  get size(): number {
    return this.children.size;
  }

  /** Whether `command` passes through: a standard method the backend passes, on a page or child session. */
  handles(command: NativeCommand, pageSession: boolean): boolean {
    if (!this.backend.passthrough || command.method.startsWith('Oya.')) return false;
    const child = !!this.children.get(command.sessionId);
    return (child || pageSession) && !!this.backend.passesThrough?.(command.method);
  }

  /** Sends `command` to its tab: a child session's to that child, a page session's to the page. */
  send(command: NativeCommand, target: () => string): Promise<unknown> {
    const child = this.children.get(command.sessionId);
    const route = child ?? { target: target(), child: '' };
    if (!child) this.watch(command.sessionId ?? route.target, route.target, command.sessionId);
    const params = this.children.paramsFor(command, child?.key ?? command.sessionId ?? route.target);
    return this.backend.passthrough!(route.target, command.method, params, route.child);
  }

  /** Starts forwarding the tab's events for one page session, once. */
  private watch(key: string, target: string, sessionId?: string): void {
    if (this.watches.has(key) || !this.backend.passthroughEvents) return;
    const watch = { active: true, stop: () => {} };
    this.watches.set(key, watch);
    this.start(watch, key, target, sessionId);
    if (!watch.active) throw Error('Page session was closed during observer setup');
  }
  /** Observer setup failure rolls back the watch before returning to the caller. */
  private start(watch: Watch, key: string, target: string, sessionId?: string): void {
    try {
      this.listen(watch, key, target, sessionId);
    } catch (error) {
      this.release(key);
      throw error;
    }
  }
  /** Synchronous revocation during observer setup still removes the returned listener. */
  private listen(watch: Watch, key: string, target: string, sessionId?: string): void {
    watch.stop = this.backend.passthroughEvents!(target, (method, params, child) => {
      if (watch.active) this.deliver(key, target, sessionId, { method, params, child });
    });
    if (!watch.active) watch.stop();
  }

  /** Delivers one tab event to the page session or the child session it came from. */
  private deliver(key: string, target: string, sessionId: string | undefined, event: TabEvent): void {
    try {
      const translated = this.children.event(key, target, sessionId, event);
      if (translated) this.emit(translated);
    } catch (error) {
      if (this.limits) this.limits.fatal(error instanceof Error ? error.message : 'Child session admission failed');
      else this.dispose();
    }
  }

  /** Stops one page session's forwarding and forgets its children. */
  release(key: string): void {
    const watch = this.watches.get(key);
    if (watch) watch.active = false;
    this.watches.delete(key);
    this.children.release(key);
    watch?.stop();
  }

  /** Stops everything this connection forwards. */
  dispose(): void {
    for (const key of [...this.watches.keys()]) this.release(key);
  }
}

/** Standalone passthrough observers retain the native connection's bounded session convention. */
function childCapacity(children: number): void {
  if (children > NATIVE_DOOR.maxSessions) throw Error('Native session limit exceeded');
}
