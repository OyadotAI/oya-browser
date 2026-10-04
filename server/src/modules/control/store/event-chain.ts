/**
 * The hash chain over each project's control events, as the SQLite backend
 * keeps it. Postgres keeps the same chain inside control_commit
 * (migrations/021_control_event_chain.sql) over the same fields in the same
 * format, the detail as each backend stores it as text.
 *
 * Each event carries the hash of the event before it in its project, so an
 * edited event, or one removed from the middle, breaks a link. Retention
 * prunes the oldest events of a project, so verification starts at the first
 * event still kept and trusts its link backwards, as the audit chain's
 * anchored window does. Plain sha256: anyone who can rewrite the table can
 * recompute every link, so this finds accidents and partial edits, not a
 * determined writer.
 */
import { createHash } from 'node:crypto';

/** An event as stored: the columns its hash covers, plus its links. */
export type ChainedEvent = {
  /** Global sequence number; orders events, not hashed (the links order them). */
  seq: number;
  /** Project the event belongs to. */
  project: string;
  /** Event type. */
  type: string;
  /** Session the event concerns, or null. */
  session_id: string | null;
  /** Milliseconds since the epoch. */
  at: number;
  /** The detail exactly as stored: JSON text. */
  detail: string;
  /** Hash of the project's previous event. */
  prev_hash?: string | null;
  /** This event's hash. */
  hash?: string | null;
};

/** What checking a project's event chain found. */
export type EventChainVerdict = {
  /** True when every kept event links to the one before it and matches its hash. */
  ok: boolean;
  /** How many events were checked before the first break. */
  checked: number;
  /** The first event that does not add up, and why. */
  broken?: EventBreak;
};

/** An event that does not follow the one before it. */
export type EventBreak = {
  /** Its sequence number. */
  seq: number;
  /** What did not add up. */
  reason: string;
};

/** The hash of an event linked to `prev`: sha256 over the fields, newline separated. */
export function eventHash(prev: string, e: Omit<ChainedEvent, 'seq'>) {
  const text = `${prev}\n${e.project}\n${e.type}\n${e.session_id ?? ''}\n${e.at}\n${e.detail}`;
  return createHash('sha256').update(text, 'utf8').digest('hex');
}

/** Why an event does not follow `expectedPrev`, or null when it does. */
function eventBreak(row: ChainedEvent, expectedPrev: string): string | null {
  if (row.prev_hash !== expectedPrev) return 'prev_hash does not match the previous event, an event was removed';
  if (row.hash !== eventHash(row.prev_hash, row)) return 'hash does not match the event, it was edited';
  return null;
}

/** Verifies chained events in ascending sequence, trusting the first one's link backwards (a pruned start). */
export function verifyEventChain(rows: ChainedEvent[]): EventChainVerdict {
  for (const [i, row] of rows.entries()) {
    const reason = eventBreak(row, i ? rows[i - 1].hash : row.prev_hash);
    if (reason) return { ok: false, checked: i, broken: { seq: Number(row.seq), reason } };
  }
  return { ok: true, checked: rows.length };
}
