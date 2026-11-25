/** Isolate the debugger's iframe and worker IDs for each external page attachment. */
import { randomUUID } from 'node:crypto';
import type { NativeCommand } from './types.ts';

/** Exact tab and debugger session behind one opaque external child attachment. */
export interface ChildRoute {
  /** Connection-local external identity, never the raw debugger ID. */
  readonly id: string;
  /** Page watch that observed this child. */
  readonly key: string;
  /** Authorized native tab identity. */
  readonly target: string;
  /** Raw debugger session used only behind the existing backend seam. */
  readonly child: string;
  /** Immediate external page or child parent. */
  readonly parent?: string;
}

/** One event from a tab's debugger, before session identities are translated. */
export interface TabEvent {
  /** Standard CDP event name. */
  readonly method: string;
  /** Event data; only documented session fields are translated. */
  readonly params: Record<string, unknown>;
  /** Raw parent session, empty for the page itself. */
  readonly child: string;
}

/** Independent raw namespaces prevent two watches from replacing each other's CDP sessions. */
export class ChildSessions {
  /** External identities resolve to exactly one watch and native tab. */
  private readonly routes = new Map<string, ChildRoute>();
  /** A raw identity grants authority only within the watch that observed it. */
  private readonly scopes = new Map<string, Map<string, ChildRoute>>();
  /** Shared browser/page/child admission supplied by the owning protocol. */
  private readonly capacity: (additional: number) => void;

  /** Reuse the connection's existing session budget. */
  constructor(capacity: (additional: number) => void) {
    this.capacity = capacity;
  }
  /** Live aliases count against the same bounded connection resources as page sessions. */
  get size(): number {
    return this.routes.size;
  }
  /** Resolve only an opaque identity issued by this connection. */
  get(id?: string): ChildRoute | undefined {
    return id ? this.routes.get(id) : undefined;
  }
  /** Translate event session fields without changing target, frame, request or object identities. */
  event(key: string, target: string, sessionId: string | undefined, event: TabEvent): object | undefined {
    const scope = this.scope(key),
      parent = event.child ? scope.get(event.child)?.id : sessionId;
    if (event.child && !scope.has(event.child)) return;
    const params = this.params({ key, target, parent }, scope, event);
    if (params) return { method: event.method, params, ...(parent ? { sessionId: parent } : {}) };
  }
  /** Attachment and detach events carry one raw child ID that must stay inside its own watch. */
  private params(owner: Omit<ChildRoute, 'id' | 'child'>, scope: Map<string, ChildRoute>, event: TabEvent) {
    if (event.method === 'Target.attachedToTarget') return this.attach(owner, scope, event.params);
    if (event.method !== 'Target.detachedFromTarget') return event.params;
    const route = typeof event.params.sessionId === 'string' ? scope.get(event.params.sessionId) : undefined;
    if (!route || route.parent !== owner.parent) return;
    this.remove(route);
    return { ...event.params, sessionId: route.id };
  }
  /** Duplicate announcements never replace a live route or recreate client-side session state. */
  private attach(
    owner: Omit<ChildRoute, 'id' | 'child'>,
    scope: Map<string, ChildRoute>,
    params: Record<string, unknown>,
  ) {
    if (typeof params.sessionId !== 'string' || scope.has(params.sessionId)) return;
    this.capacity(this.routes.size + 1);
    return { ...params, sessionId: this.allocate(owner, params.sessionId) };
  }
  /** Register both lookup directions together after successful capacity admission. */
  private allocate(owner: Omit<ChildRoute, 'id' | 'child'>, child: string): string {
    const route = { ...owner, child, id: randomUUID() };
    this.scope(route.key).set(route.child, route);
    this.routes.set(route.id, route);
    return route.id;
  }
  /** Only the immediate parent can address an existing child for detach. */
  paramsFor(command: NativeCommand, key: string): Record<string, unknown> {
    if (command.method !== 'Target.detachFromTarget') return command.params;
    const route = this.get(typeof command.params.sessionId === 'string' ? command.params.sessionId : undefined);
    if (!route || route.key !== key || route.parent !== command.sessionId)
      throw Error('Child session is closed, unavailable, or outside this parent');
    return { ...command.params, sessionId: route.child };
  }
  /** A watch receives an empty namespace until it observes its own attachment events. */
  private scope(key: string): Map<string, ChildRoute> {
    if (!this.scopes.has(key)) this.scopes.set(key, new Map());
    return this.scopes.get(key)!;
  }
  /** Revoke descendants first so stale worker aliases cannot outlive a detached frame. */
  private remove(route: ChildRoute): void {
    for (const child of this.scope(route.key).values()) if (child.parent === route.id) this.remove(child);
    this.scope(route.key).delete(route.child);
    this.routes.delete(route.id);
  }
  /** Page cleanup forgets only this watch's aliases, including nested workers. */
  release(key: string): void {
    for (const route of this.scopes.get(key)?.values() ?? []) this.routes.delete(route.id);
    this.scopes.delete(key);
  }
}
