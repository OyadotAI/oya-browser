/**
 * The SQL the SQLite backend runs: its schema and statements. Rows are versioned
 * bodies; `control_gates` mirrors each session's control state so command
 * admission never rewrites session rows.
 */

/** Schema, created on open; WAL with full sync, and a busy timeout for the rare second reader. */
export const SCHEMA = `PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS control_rows (kind TEXT NOT NULL, id TEXT NOT NULL, project TEXT, state TEXT, expires_at INTEGER, body TEXT NOT NULL, version INTEGER NOT NULL, PRIMARY KEY (kind, id));
      CREATE INDEX IF NOT EXISTS control_rows_project ON control_rows (kind, project);
      CREATE INDEX IF NOT EXISTS control_rows_state ON control_rows (kind, state);
      CREATE INDEX IF NOT EXISTS control_rows_expiry ON control_rows (expires_at) WHERE expires_at IS NOT NULL;
      CREATE TABLE IF NOT EXISTS control_events (seq INTEGER PRIMARY KEY AUTOINCREMENT, project TEXT NOT NULL, type TEXT NOT NULL, session_id TEXT, at INTEGER NOT NULL, detail TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS control_events_project ON control_events (project, seq);
      CREATE TABLE IF NOT EXISTS control_gates (id TEXT PRIMARY KEY, state TEXT NOT NULL, mode TEXT NOT NULL, holder TEXT, expiresAt INTEGER, fence INTEGER NOT NULL, inFlight INTEGER NOT NULL DEFAULT 0, owner TEXT);`;

/** Rows with their session gate's in-flight count; the caller appends the WHERE clause. */
export const SELECT_ROWS =
  "SELECT r.id, r.version, r.body, g.inFlight FROM control_rows r LEFT JOIN control_gates g ON r.kind = 'session' AND g.id = r.id";
/** One row's current version, for compare-and-swap. */
export const SELECT_VERSION = 'SELECT version FROM control_rows WHERE kind = ? AND id = ?';
/** Delete one row. */
export const DELETE_ROW = 'DELETE FROM control_rows WHERE kind = ? AND id = ?';
/** Delete a session's gate with its row. */
export const DELETE_GATE = 'DELETE FROM control_gates WHERE id = ?';
/** Insert a row at version 1, or replace it and bump its version. */
export const UPSERT_ROW =
  'INSERT INTO control_rows VALUES (?,?,?,?,?,?,1) ON CONFLICT (kind, id) DO UPDATE SET project = excluded.project, state = excluded.state, expires_at = excluded.expires_at, body = excluded.body, version = version + 1';
/** A gate's mode and in-flight count. */
export const SELECT_GATE_MODE = 'SELECT mode, inFlight FROM control_gates WHERE id = ?';
/** A whole gate. */
export const SELECT_GATE = 'SELECT * FROM control_gates WHERE id = ?';
/** Mirror a session into its gate; a new fence (the session was re-placed) zeroes the in-flight count. */
export const UPSERT_GATE =
  'INSERT INTO control_gates VALUES (?,?,?,?,?,?,0,?) ON CONFLICT (id) DO UPDATE SET owner = excluded.owner, state = excluded.state, mode = excluded.mode, holder = excluded.holder, expiresAt = excluded.expiresAt, inFlight = CASE WHEN fence <> excluded.fence THEN 0 ELSE inFlight END, fence = excluded.fence';
/** Count an admitted command in flight. */
export const BEGIN_COMMAND = 'UPDATE control_gates SET inFlight = inFlight + 1 WHERE id = ?';
/** Settle a command admitted under the gate's current fence. */
export const FINISH_COMMAND = 'UPDATE control_gates SET inFlight = max(0, inFlight - 1) WHERE id = ? AND fence = ?';
/** Append one event. */
export const INSERT_EVENT = 'INSERT INTO control_events (project, type, session_id, at, detail) VALUES (?,?,?,?,?)';
/** A project's webhooks. */
export const SELECT_WEBHOOKS = "SELECT body FROM control_rows WHERE kind = 'webhook' AND project = ?";
/** Queue a pending delivery. */
export const INSERT_DELIVERY = "INSERT INTO control_rows VALUES ('delivery', ?, ?, 'pending', NULL, ?, 1)";
/** A project's latest events, newest first. */
export const SELECT_LATEST_EVENTS = 'SELECT * FROM control_events WHERE project = ? ORDER BY seq DESC LIMIT ?';
/** A project's events after a sequence number. */
export const SELECT_EVENTS_AFTER = 'SELECT * FROM control_events WHERE project = ? AND seq > ? ORDER BY seq LIMIT ?';
/** Gates of sessions whose rows are about to expire. */
export const DELETE_EXPIRED_GATES =
  "DELETE FROM control_gates WHERE id IN (SELECT id FROM control_rows WHERE kind = 'session' AND expires_at < ?)";
/** Rows past their expiry. */
export const DELETE_EXPIRED_ROWS = 'DELETE FROM control_rows WHERE expires_at < ?';
/** A project's events older than its cutoff. */
export const DELETE_OLD_EVENTS = 'DELETE FROM control_events WHERE project = ? AND at < ?';

/** `?,?,…` for an IN list of `values`. */
export const placeholders = (values) => values.map(() => '?').join(',');
