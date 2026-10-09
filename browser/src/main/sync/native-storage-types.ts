/** Explicit browser-owned storage operations; no debugging protocol or page-evaluation capability is accepted. */
import type { Origins } from '../../page/login-state.ts';
/** Session-scoped native observer notifications contain no credential values. */
export type StorageListener = (event: unknown, origin: string, available: boolean) => void;
/** The native engine seam, bound to exactly one persona's session. */
export interface NativeStoragePort {
  /** Read a bounded first-party origin snapshot. */
  _readOyaLocalStorage(origin: string): Promise<string[][]>;
  /** Restore only during exclusive initialization, before any page may use the partition. */
  _restoreOyaLocalStorage(origin: string, entries: string[][]): Promise<string[][]>;
  /** Resolve when the native observer is actually installed. */
  _watchOyaLocalStorage(origin: string): Promise<void>;
  /** Cancel this exclusive owner's observer and any pending readiness. */
  _unwatchOyaLocalStorage(origin: string): void;
  /** Subscribe only to this session's native events. */
  on(event: 'oya-local-storage-changed', listener: StorageListener): unknown;
  /** Remove the same listener on persona disposal. */
  removeListener(event: 'oya-local-storage-changed', listener: StorageListener): unknown;
}
/** The transport returns acceptance only; durable persistence requires the server's separate flush acknowledgment. */
export interface StorageSyncDeps {
  /** An immutable session for one persona; never a getter that switches identity mid-await. */
  session: NativeStoragePort;
  /** Whether this persona's authenticated socket may receive snapshots. */
  ready(): boolean;
  /** False means keep the snapshot for reconnect. */
  send(message: StorageMessage): boolean;
}

/** One partition's latest snapshots, including empty logout dictionaries. */
export interface StorageMessage {
  /** Route the update to profile storage persistence. */
  type: 'storage_changed';
  /** First-party origins captured by this immutable session. */
  origins: Origins;
}
