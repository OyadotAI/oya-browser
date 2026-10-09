/** One persona's native storage synchronization lifecycle, with offline retention and disposal fencing. */
import { withinTime } from '../../shared/within-time.ts';
import type { Origins } from '../../page/login-state.ts';
import type { NativeStoragePort, StorageListener, StorageSyncDeps } from './native-storage-types.ts';
import { storageImport, storageOrigin, storageValues } from './native-storage-values.ts';
import { STORAGE_CAPTURE_TIMEOUT_MS, STORAGE_ORIGINS_MAX } from './constants.ts';
export type { NativeStoragePort, StorageSyncDeps } from './native-storage-types.ts';
/** Fail explicitly on older engines rather than falling back to CDP or late page-script injection. */
function requireNative(session: NativeStoragePort): void {
  for (const name of [
    '_readOyaLocalStorage',
    '_restoreOyaLocalStorage',
    '_watchOyaLocalStorage',
    '_unwatchOyaLocalStorage',
  ])
    if (typeof session[name as keyof NativeStoragePort] !== 'function')
      throw Error('Native storage capability unavailable');
}
/** Exclusive ownership: exactly one instance watches storage for a persona's session. */
export class NativeStorageSync {
  /** Never retarget this object to another persona's session. */
  private readonly deps: StorageSyncDeps;
  /** Observer readiness is shared when two tabs discover the same origin. */
  private readonly watching = new Map<string, Promise<void>>();
  /** Native mutations coalesce into one reader per origin. */
  private readonly reading = new Map<string, Promise<void>>();
  /** Mutations arriving during a native read require another snapshot. */
  private readonly dirty = new Set<string>();
  /** Latest snapshots retained until the transport accepts them, including empty logout state. */
  private readonly unsent = new Map<string, Record<string, string>>();
  /** A discarded persona can never publish a late native reply. */
  private closed = false;
  /** Failed native capture must be reported by flush, never mistaken for a saved profile. */
  private failure: Error | null = null;
  /** Serialize exclusive initialization; no concurrent restore can race an empty store check. */
  private initialization?: Promise<void>;
  /** The exact listener removed on disposal. */
  private readonly changed: StorageListener = (_event, origin, available) => this.onChange(origin, available);
  /** Construction validates native capability without publishing or mutating storage. */
  constructor(deps: StorageSyncDeps) {
    requireNative(deps.session);
    this.deps = deps;
    deps.session.on('oya-local-storage-changed', this.changed);
  }
  /** Only call while the caller exclusively holds the partition before navigation. */
  initialize(origins: Origins): Promise<void> {
    this.assertOpen();
    if (this.initialization) throw Error('Native storage initialization already started');
    const entries = storageImport(origins);
    this.initialization = this.restore(entries);
    return this.initialization;
  }
  /** Complete validated restoration before any observer capture is considered ready. */
  private async restore(entries: [string, string[][]][]): Promise<void> {
    for (const [origin, values] of entries) {
      this.assertOpen();
      await this.bounded(() => this.deps.session._restoreOyaLocalStorage(origin, values));
      this.assertOpen();
      await this.watch(origin);
    }
  }
  /** Discover one first-party origin without restoring it again after a logout. */
  watch(origin: string): Promise<void> {
    this.assertOpen();
    storageOrigin(origin);
    const existing = this.watching.get(origin);
    if (existing) return existing;
    if (this.watching.size >= STORAGE_ORIGINS_MAX) throw Error('Too many native storage origins');
    const ready = this.startWatch(origin);
    this.watching.set(origin, ready);
    return ready;
  }
  /** Read once after observer installation to cover changes that happened before subscription. */
  private async startWatch(origin: string): Promise<void> {
    try {
      await this.bounded(() => this.deps.session._watchOyaLocalStorage(origin));
      this.assertOpen();
      await this.capture(origin);
    } catch {
      this.failure = Error('Native storage observation failed');
      throw this.failure;
    }
  }
  /** Ignore foreign/late events; loss of observation is a hard sync failure. */
  private onChange(origin: string, available: boolean): void {
    if (this.closed || !this.watching.has(origin)) return;
    if (!available) {
      this.failure = Error('Native storage observer disconnected');
      return;
    }
    void this.capture(origin).catch(() => {
      this.failure = Error('Native storage capture failed');
    });
  }
  /** A native read already in flight must loop when a later mutation invalidates it. */
  private capture(origin: string): Promise<void> {
    this.dirty.add(origin);
    const existing = this.reading.get(origin);
    if (existing) return existing;
    const task = this.drain(origin).finally(() => this.reading.delete(origin));
    this.reading.set(origin, task);
    return task;
  }
  /** Bound a continuously changing page rather than allowing it to hold a save forever. */
  private async drain(origin: string): Promise<void> {
    const deadline = Date.now() + STORAGE_CAPTURE_TIMEOUT_MS;
    while (this.dirty.delete(origin)) {
      this.assertOpen();
      if (Date.now() >= deadline) throw Error('Native storage capture timed out');
      await this.snapshot(origin, deadline);
    }
  }
  /** Check ownership again after a native read, before retaining any values. */
  private async snapshot(origin: string, deadline: number): Promise<void> {
    const entries = await this.bounded(() => this.deps.session._readOyaLocalStorage(origin), deadline - Date.now());
    this.assertOpen();
    this.unsent.set(origin, storageValues(entries));
  }
  /** Capture all watched origins and deliver retained snapshots; not a durable server acknowledgment. */
  async flush(): Promise<boolean> {
    this.assertOpen();
    if (this.failure) throw this.failure;
    await this.initialization;
    await Promise.all(this.watching.values());
    await Promise.all([...this.watching.keys()].map((origin) => this.capture(origin)));
    this.assertOpen();
    if (this.failure) throw this.failure;
    return this.deliver();
  }
  /** Deliver captured mutations without periodically rescanning every unchanged origin. */
  async flushPending(): Promise<boolean> {
    this.assertOpen();
    await this.initialization;
    await Promise.all(this.watching.values());
    await Promise.all(this.reading.values());
    this.assertOpen();
    if (this.failure) throw this.failure;
    return this.deliver();
  }
  /** A refused or throwing send retains the latest state for reconnect, including empty dictionaries. */
  private deliver(): boolean {
    if (!this.deps.ready()) return false;
    const pending = new Map(this.unsent);
    if (!pending.size) return true;
    if (!this.trySend(pending)) return false;
    for (const [origin, values] of pending) if (this.unsent.get(origin) === values) this.unsent.delete(origin);
    return true;
  }
  /** Transport errors are retryable and must not consume pending snapshots. */
  private trySend(pending: Map<string, Record<string, string>>): boolean {
    try {
      return this.deps.send({ type: 'storage_changed', origins: Object.fromEntries(pending) });
    } catch {
      return false;
    }
  }
  /** Bound every native wait and sanitize errors so storage values cannot leak through diagnostics. */
  private bounded<T>(operation: () => Promise<T>, ms = STORAGE_CAPTURE_TIMEOUT_MS): Promise<T> {
    const task = Promise.resolve().then(() => {
      this.assertOpen();
      return operation();
    });
    return withinTime(task, ms, 'Native storage operation timed out').catch(() => this.operationFailed());
  }
  /** Preserve disposal semantics without exposing the engine's potentially sensitive error text. */
  private operationFailed(): never {
    this.assertOpen();
    this.failure = Error('Native storage operation failed');
    throw this.failure;
  }
  /** Refuse all operations after persona disposal. */
  private assertOpen(): void {
    if (this.closed) throw Error('Native storage sync is disposed');
  }
  /** Tear down only this instance's observers; late reads remain unable to publish into another persona. */
  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    this.deps.session.removeListener('oya-local-storage-changed', this.changed);
    for (const origin of this.watching.keys()) this.deps.session._unwatchOyaLocalStorage(origin);
    this.watching.clear();
    this.unsent.clear();
    this.dirty.clear();
  }
}
