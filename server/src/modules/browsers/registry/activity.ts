/**
 * Keeping a browser's command log: counters, the last error, and a bounded,
 * newest-first list of what it did.
 */
import { ACTIVITY_SIZE, ERROR_CHARS } from '../constants.ts';

/** A failed command: counted, and its error kept (trimmed). */
function noteError(b, error) {
  b.errors++;
  b.lastError = String(error || 'failed').slice(0, ERROR_CHARS);
}

/** One activity entry; a failed one carries its error. */
const entry = (b, action, summary, ok, ms) => ({
  ts: b.lastCommandAt.toISOString(),
  action,
  summary,
  ok: !!ok,
  ms: Math.round(ms),
  ...(ok ? {} : { error: b.lastError }),
});

/** Applies one settled command to the record `b`. */
export function applyActivity(b, { action, summary = '', ok, ms = 0, error = null }) {
  b.pending = Math.max(0, b.pending - 1);
  b.commands++;
  if (!ok) noteError(b, error);
  b.lastCommandAt = new Date();
  b.lastSeen = b.lastCommandAt;
  b.activity.unshift(entry(b, action, summary, ok, ms));
  if (b.activity.length > ACTIVITY_SIZE) b.activity.length = ACTIVITY_SIZE;
}
