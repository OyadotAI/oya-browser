/**
 * The durable control plane on plain Postgres, without Supabase.
 *
 * RemoteBackend (see store.js) speaks to exactly six functions, by name, with
 * named arguments — which is all PostgREST was ever giving it. So the entire
 * Supabase dependency for control storage is this file: a { rpc } object of the
 * same shape, talking to Postgres directly.
 *
 * Apply the schema first with `node server/migrations/run.mjs`.
 */

import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { DEFAULT_POOL_MAX, PG_CONNECT_TIMEOUT_MS, PG_IDLE_TIMEOUT_MS } from './store/constants.ts';

let pool = null;

/** The pg module, which only this path needs, loaded on demand like the cloud SDK. */
async function loadPg() {
  try {
    return (await import('pg')).default;
  } catch {
    throw new HttpError(Status.UNAVAILABLE, 'DATABASE_URL is set but the "pg" package is not installed', {
      code: 'storage_unavailable',
    });
  }
}

/** pg is only needed on this path, so it loads on demand, like the cloud SDK. */
async function getPool(connectionString) {
  if (pool) return pool;
  pool = new (await loadPg()).Pool(poolOptions(connectionString));
  // A pool that emits an unhandled 'error' takes the process down with it.
  pool.on('error', (err) => console.error('[pg] idle client error:', err.message));
  return pool;
}

/** Pool settings: size from DATABASE_POOL_MAX, and bounded connect and idle times. */
function poolOptions(connectionString) {
  return {
    connectionString,
    max: Number(process.env.DATABASE_POOL_MAX || DEFAULT_POOL_MAX),
    // The control store is on the request path; a hung connect must not hang a request.
    connectionTimeoutMillis: PG_CONNECT_TIMEOUT_MS,
    idleTimeoutMillis: PG_IDLE_TIMEOUT_MS,
  };
}

/**
 * jsonb arguments must be sent as JSON text. This matters most for arrays:
 * node-postgres renders a JS array as a Postgres array literal ({a,b}), not as
 * JSON, so `writes` and `events` would arrive malformed without this.
 */
const toParam = (value) => (value !== null && typeof value === 'object' ? JSON.stringify(value) : value);

/** `select oya_browser.name(a => $1, …)`: named notation, so argument order here never has to track the SQL. */
const callSql = (name, keys) => `select oya_browser.${name}(${keys.map((k, i) => `${k} => $${i + 1}`).join(', ')})`;

/** The single value one control function returns. */
async function query(connectionString, name, args) {
  const keys = Object.keys(args);
  const db = await getPool(connectionString);
  const result = await db.query(
    callSql(name, keys),
    keys.map((k) => toParam(args[k])),
  );
  // One function, one column, named after the function.
  const row = result.rows[0];
  return row ? (row[name] ?? null) : null;
}

/** Call one control function and return its single value in Supabase's `{ data }` / `{ error }` shape. */
async function rpc(connectionString, name, args) {
  try {
    return { data: await query(connectionString, name, args) };
  } catch (error) {
    // RemoteBackend.call() reads these messages to tell a conflict from an
    // outage, so pass the text through rather than wrapping it.
    return { error: { message: String(error.message || error) } };
  }
}

/**
 * @param {string} [connectionString]
 * @returns {{ rpc: (name: string, args?: Record<string, unknown>) => Promise<{ data?: unknown, error?: { message: string } }> }|null}
 */
export function pgRemote(connectionString = process.env.DATABASE_URL) {
  if (!connectionString) return null;
  return {
    /** Call oya_browser.<name> with named arguments. */
    rpc: (name, args = {}) => rpc(connectionString, name, args),
  };
}

/** Close the Postgres pool, if one was opened. */
export async function closePgPool() {
  await pool?.end();
  pool = null;
}
