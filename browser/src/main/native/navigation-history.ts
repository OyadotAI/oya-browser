/** Connection-scoped history snapshots expose native entries without leaking serialized page state. */
import { createHash, randomUUID } from 'node:crypto';
import type { WebContents } from 'electron';
import type { NativePage } from './page.ts';
import { NATIVE_HISTORY_ENTRIES, NATIVE_HISTORY_BYTES } from './constants.ts';
/** Only the most recent snapshot for an exact native page may authorize traversal. */
interface HistoryLease {
  /** Unguessable connection-local snapshot capability. */
  token: string;
  /** Digest includes native serialized state, never sent to the agent. */
  digest: string;
}
/** Native history is tab-local, not the browser's global browsing-history database. */
export class NativeNavigationHistory {
  /** Weak ownership never retains a closed tab or shares tokens between sockets. */
  private readonly leases = new WeakMap<WebContents, HistoryLease>();
  /** Read actual browser-owned navigation state, with no synthetic CDP entry identities. */
  read(page: NativePage): object {
    const state = historyState(page.webContents),
      token = randomUUID();
    this.leases.set(page.webContents, { token, digest: state.digest });
    return { snapshot: token, currentIndex: state.currentIndex, entries: state.entries };
  }
  /** Verify the whole snapshot and destination policy before native history traversal. */
  navigate(page: NativePage, params: Record<string, unknown>, allowed: (url: string) => boolean): object {
    const wc = page.webContents,
      state = historyState(wc);
    validateHistoryDestination(params, state.entries.length);
    requireHistoryLease(this.leases.get(wc), params.snapshot, state.digest);
    if (!allowed(state.entries[params.index as number].url)) throw Error('History destination is not authorized');
    this.leases.delete(wc);
    wc.navigationHistory.goToIndex(params.index as number);
    return {};
  }
}
/** Native reads and traversal are synchronous, preventing an interleaved page command from retargeting them. */
function historyState(wc: WebContents) {
  if (wc.isDestroyed()) throw Error('View is destroyed');
  const entries = wc.navigationHistory.getAllEntries(),
    currentIndex = wc.navigationHistory.getActiveIndex();
  if (entries.length > NATIVE_HISTORY_ENTRIES) throw Error('Native history entry limit exceeded');
  const serialized = JSON.stringify({ entries, currentIndex });
  if (Buffer.byteLength(serialized) > NATIVE_HISTORY_BYTES) throw Error('Native history state limit exceeded');
  const digest = createHash('sha256').update(serialized).digest('hex');
  return { digest, currentIndex, entries: entries.map(({ url, title }, index) => ({ index, url, title })) };
}
/** Indexes are positions in a verified snapshot, never silently coerced or treated as stable entry IDs. */
function validateHistoryDestination(params: Record<string, unknown>, count: number): void {
  if (typeof params.snapshot !== 'string' || !params.snapshot) throw Error('snapshot is required');
  if (!Number.isSafeInteger(params.index) || (params.index as number) < 0 || (params.index as number) >= count)
    throw Error('Invalid native history index');
}

/** Reject stale or foreign capabilities before checking the destination or consuming native history. */
function requireHistoryLease(lease: HistoryLease | undefined, token: unknown, digest: string): void {
  if (!lease || lease.token !== token || lease.digest !== digest)
    throw Error('Unknown, foreign or changed native history snapshot');
}
