/**
 * The Postgres driver: every table in the oya_browser schema, through one pg
 * pool on DATABASE_URL. The schema comes from server/migrations (run.mjs),
 * which the container applies on every start.
 */
import { HttpError } from '../errors.ts';
import { Status } from '../http-status.ts';
import { DB_BATCH_ROWS, DEFAULT_POOL_MAX, PG_CONNECT_TIMEOUT_MS, PG_IDLE_TIMEOUT_MS } from '../constants.ts';
import { rowFromStored, TABLES } from './schema.ts';
import { whereSql, tailSql, conflictSql, setSql, type Bind } from './sql.ts';
import type { Connection, ControlRemote, Row, SelectOptions, Where } from './connection.ts';

/** The process-wide pg pool, opened on first use and shared by every table and the control plane. */
let pool = null;

/** The pg module, which only this driver needs, loaded on demand. */
async function loadPg() {
  try {
    return (await import('pg')).default;
  } catch {
    throw new HttpError(Status.UNAVAILABLE, 'Postgres storage is configured but the "pg" package is not installed', {
      code: 'storage_unavailable',
    });
  }
}

/** The pool, opened on first use. */
async function getPool(connectionString: string) {
  if (pool) return pool;
  pool = new (await loadPg()).Pool(poolOptions(connectionString));
  // A pool that emits an unhandled 'error' takes the process down with it.
  pool.on('error', (err) => console.error('[pg] idle client error:', err.message));
  return pool;
}

/** Pool settings: size from DATABASE_POOL_MAX, and bounded connect and idle times. */
function poolOptions(connectionString: string) {
  return {
    connectionString,
    max: Number(process.env.DATABASE_POOL_MAX || DEFAULT_POOL_MAX),
    // Storage is on the request path; a hung connect must not hang a request.
    connectionTimeoutMillis: PG_CONNECT_TIMEOUT_MS,
    idleTimeoutMillis: PG_IDLE_TIMEOUT_MS,
  };
}

/** Close the pool, if one was opened. */
export async function closePgPool() {
  await pool?.end();
  pool = null;
}

/** Rows of a parameterized statement. */
const run = async (connectionString: string, sql: string, params: unknown[]) =>
  (await (await getPool(connectionString)).query(sql, params)).rows;

/**
 * A statement builder's `bind`: $1, $2, … with the values collected. Objects go
 * as JSON text, because node-postgres renders a JS array as a Postgres array
 * literal ({a,b}), not as JSON, and a jsonb column or argument would arrive malformed.
 */
function binder() {
  const params: unknown[] = [];
  const bind: Bind = (value) =>
    `$${params.push(value !== null && typeof value === 'object' ? JSON.stringify(value) : value)}`;
  return { params, bind };
}

/** Where a table lives. */
const qualified = (table: string) => `oya_browser.${table}`;

/** One control function call: named notation, so argument order never has to track the SQL. */
async function callControl(connectionString: string, name: string, args: Record<string, unknown>) {
  const { params, bind } = binder();
  const named = Object.entries(args).map(([k, v]) => `${k} => ${bind(v)}`);
  const [row] = await run(connectionString, `select oya_browser.${name}(${named.join(', ')})`, params);
  // One function, one column, named after the function.
  return row ? (row[name] ?? null) : null;
}

/**
 * The control plane's `{ rpc }` client over Postgres, answering `{ data }` or
 * `{ error }`. RemoteBackend reads the error text to tell a conflict from an
 * outage, so it passes through unwrapped. Null without a connection string.
 */
export function pgRemote(connectionString = process.env.DATABASE_URL): ControlRemote | null {
  if (!connectionString) return null;
  return {
    /** Call oya_browser.<name> with named arguments. */
    rpc: (name, args = {}) => rpcResult(() => callControl(connectionString, name, args)),
  };
}

/** A call's value as `{ data }`, or its failure's text as `{ error }`. */
async function rpcResult(call: () => Promise<unknown>) {
  try {
    return { data: await call() };
  } catch (error) {
    return { error: { message: String(error.message || error) } };
  }
}

/** Postgres behind the storage contract. */
export class PostgresConnection implements Connection {
  /** Which driver this is. */
  readonly kind = 'postgres' as const;
  /** Where the database is. */
  declare readonly url: string;
  /** A driver on the database at `url`. */
  constructor(url: string) {
    this.url = url;
  }

  /** Rows of `table` matching `where`, sorted and cut as asked. */
  async select(table: string, where: Where = {}, options: SelectOptions = {}) {
    const { params, bind } = binder();
    const sql = `select * from ${qualified(table)}${whereSql(where, bind)}${tailSql(options, bind)}`;
    return (await run(this.url, sql, params)).map((row) => rowFromStored(table, row));
  }

  /** Inserts rows; with `update`, replaces a row whose key is already stored. */
  async upsert(table: string, rows: Row[], { update = false } = {}) {
    for (let i = 0; i < rows.length; i += DB_BATCH_ROWS)
      await this.insertChunk(table, rows.slice(i, i + DB_BATCH_ROWS), update);
  }

  /** One multi-row insert; the columns are those the first row names, and the rest keep their defaults. */
  private async insertChunk(table: string, rows: Row[], update: boolean) {
    const columns = Object.keys(rows[0]);
    const { params, bind } = binder();
    const values = rows.map((row) => `(${columns.map((c) => bind(row[c] ?? null)).join(', ')})`);
    const into = `insert into ${qualified(table)} (${columns.join(', ')}) values ${values.join(', ')}`;
    await run(this.url, into + conflictSql(TABLES[table]?.key ?? [], columns, update), params);
  }

  /** Sets `set` on the matching rows; answers how many changed. */
  async update(table: string, where: Where, set: Row) {
    const { params, bind } = binder();
    const sql = `update ${qualified(table)} set ${setSql(set, bind)}${whereSql(where, bind)} returning 1`;
    return (await run(this.url, sql, params)).length;
  }

  /** Deletes the matching rows; answers how many went. */
  async delete(table: string, where: Where) {
    const { params, bind } = binder();
    return (await run(this.url, `delete from ${qualified(table)}${whereSql(where, bind)} returning 1`, params)).length;
  }

  /** Whether the table exists in this storage. */
  async exists(table: string) {
    const [row] = await run(this.url, 'select to_regclass($1) is not null as found', [qualified(table)]);
    return Boolean(row?.found);
  }

  /** The control plane's remote client, or null to keep it in its own SQLite file. */
  controlRemote() {
    return pgRemote(this.url);
  }

  /** Releases what the driver holds. */
  async close() {
    await closePgPool();
  }
}
