/** One authenticated connection owns a bounded hierarchy of browser and exact-page CDP sessions. */
import { randomUUID } from 'node:crypto';
import { NATIVE_DOOR } from './constants.ts';

/** A browser attachment has no page target; descendants retain their exact parent scope. */
export interface NativeSession {
  /** Opaque identity issued by this connection. */
  readonly id: string;
  /** Browser attachment that owns this session, or the primary browser endpoint. */
  readonly parent?: string;
  /** Authorized native page identity; absent for browser sessions. */
  readonly target?: string;
}

/** Session kinds and capacity share one registry rather than one budget per browser attachment. */
export class NativeSessions {
  /** Live connection-owned attachment records. */
  private readonly records = new Map<string, NativeSession>();

  /** Whether this exact connection issued a still-live session. */
  has(id: string): boolean {
    return this.records.has(id);
  }
  /** A stale or foreign identity never becomes browser authority. */
  get(id: string): NativeSession {
    const record = this.records.get(id);
    if (!record) throw Error('Session is closed, unavailable, or outside this endpoint');
    return record;
  }
  /** Browser scope is explicit: the primary endpoint or an issued browser attachment. */
  browser(id?: string): boolean {
    return id === undefined || !this.get(id).target;
  }
  /** Automatic attachment preflights the same budget as manual browser/page attachment. */
  requireCapacity(additional: number): void {
    if (this.records.size + additional > NATIVE_DOOR.maxSessions) throw Error('Native session limit exceeded');
  }
  /** Allocate only under a live browser scope; native target validation belongs to the protocol. */
  allocate(parent?: string, target?: string): NativeSession {
    this.requireCapacity(1);
    if (!this.browser(parent)) throw Error('Browser methods require the browser endpoint');
    const record = { id: randomUUID(), parent, target };
    this.records.set(record.id, record);
    return record;
  }
  /** Discovery reports attachment state for the browser session that requested it. */
  attached(target: string, parent?: string): boolean {
    return [...this.records.values()].some((record) => record.target === target && record.parent === parent);
  }
  /** Snapshot every page attachment, across independent browser scopes. */
  pages(): NativeSession[] {
    return [...this.records.values()].filter((record) => record.target !== undefined);
  }
  /** Descendants precede parents so detach events still address live parent scopes. */
  tree(id: string): NativeSession[] {
    const children = [...this.records.values()].filter((record) => record.parent === id);
    return [...children.flatMap((record) => this.tree(record.id)), this.get(id)];
  }
  /** Revoke before resource cleanup to make observer callbacks and repeated cleanup harmless. */
  delete(id: string): void {
    this.records.delete(id);
  }
  /** Disconnect invalidates every remaining attachment. */
  clear(): void {
    this.records.clear();
  }
}
