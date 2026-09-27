/**
 * Counts downloads and update checks per day for the admin page. Counted in
 * memory and written once a minute, never a write per download.
 */
import { DAY_CHARS, DOWNLOAD_FLUSH_MS } from './constants.ts';
import { addDownloads } from './repository.ts';

/** "day|kind|platform" -> downloads not written yet. */
const pending = new Map<string, number>();

/** Counts one download (or update check) of `kind` for `platform`, today. */
export function countDownload(kind: string, platform: string, now = new Date()) {
  const key = `${now.toISOString().slice(0, DAY_CHARS)}|${kind}|${platform}`;
  pending.set(key, (pending.get(key) || 0) + 1);
}

/** Writes the pending counts; a failed write puts them back for the next time. */
export async function flushDownloads() {
  if (!pending.size) return;
  const batch = new Map(pending);
  pending.clear();
  await addDownloads(batch).catch((e) => {
    for (const [key, n] of batch) pending.set(key, (pending.get(key) || 0) + n);
    console.error(`[admin] download counts not written (${e.message}); retrying next time`);
  });
}

const timer = setInterval(() => void flushDownloads(), DOWNLOAD_FLUSH_MS);
timer.unref?.();

/** Stops the timer and writes what is left, for shutdown. */
export async function drainDownloads() {
  clearInterval(timer);
  await flushDownloads();
}
