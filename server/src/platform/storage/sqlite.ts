/**
 * The SQLite driver: every table in one file in the data directory, created
 * from schema.ts on open. A single writer, so one replica. The control plane
 * keeps its own SQLite file (control.sqlite) with its own writer lock.
 */
import { DatabaseSync } from 'node:sqlite';
import { mkdirSync, chmodSync } from 'node:fs';
import { dirname } from 'node:path';
import { TABLES, columnsOf, rowFromStored, type ColumnType } from './schema.ts';
import { whereSql, tailSql, conflictSql, setSql, type Bind } from './sql.ts';
import { PRIVATE_DIR_MODE, PRIVATE_FILE_MODE } from './constants.ts';
import type { Connection, Row, SelectOptions, Where } from './connection.ts';

/** SQLite's type for each column type; times are ISO text, json is text. */
const AFFINITY: Record<ColumnType, string> = {
  text: 'text',
  int: 'integer',
  bool: 'integer',
  json: 'text',
  time: 'text',
  serial: 'integer',
};

/** `create table if not exists` for one table in the schema. */
function createSql(name: string) {
  const { columns, key } = TABLES[name];
  const serial = Object.entries(columns).find(([, type]) => type === 'serial')?.[0];
  const defs = Object.entries(columns).map(([c, type]) =>
    c === serial ? `${c} integer primary key autoincrement` : `${c} ${AFFINITY[type]}`,
  );
  const primary = serial ? '' : `, primary key (${key.join(', ')})`;
  return `create table if not exists ${name} (${defs.join(', ')}${primary})`;
}

/** A value as SQLite takes it: json as text, booleans as 0 or 1, undefined as null. */
function toStored(type: ColumnType | undefined, value: unknown) {
  if (value === undefined || value === null) return null;
  if (type === 'json') return JSON.stringify(value);
  if (type === 'bool') return value ? 1 : 0;
  return value as any;
}

/** A statement builder's `bind`: ? with the values collected. */
function binder() {
  const params: any[] = [];
  const bind: Bind = (value) => (params.push(value), '?');
  return { params, bind };
}

/** Whether a filter value is a bound ({ gte }) or a list ({ notIn }) rather than a plain value. */
const isTest = (value: unknown) => value !== null && typeof value === 'object' && ('gte' in value || 'notIn' in value);

/** SQLite behind the storage contract. */
export class SqliteConnection implements Connection {
  /** Which driver this is. */
  readonly kind = 'sqlite' as const;
  /** The open database. */
  declare private db: DatabaseSync;
  /** A driver on the storage at `path`. */
  constructor(path: string) {
    mkdirSync(dirname(path), { recursive: true, mode: PRIVATE_DIR_MODE });
    this.db = new DatabaseSync(path);
    chmodSync(path, PRIVATE_FILE_MODE);
    for (const name of Object.keys(TABLES)) this.db.exec(createSql(name));
  }

  /** Rows of `table` matching `where`, sorted and cut as asked. */
  async select(table: string, where: Where = {}, options: SelectOptions = {}) {
    const { params, bind } = binder();
    const sql = `select * from ${table}${whereSql(this.stored(table, where), bind)}${tailSql(options, bind)}`;
    return this.db
      .prepare(sql)
      .all(...params)
      .map((row) => rowFromStored(table, row as Row));
  }

  /** Inserts rows; with `update`, replaces a row whose key is already stored. */
  async upsert(table: string, rows: Row[], { update = false } = {}) {
    if (!rows.length) return;
    const columns = Object.keys(rows[0]);
    const types = columnsOf(table);
    const sql = `insert into ${table} (${columns.join(', ')}) values (${columns.map(() => '?').join(', ')})`;
    const statement = this.db.prepare(sql + conflictSql(TABLES[table].key, columns, update));
    this.transaction(() => rows.forEach((row) => statement.run(...columns.map((c) => toStored(types[c], row[c])))));
  }

  /** Sets `set` on the matching rows; answers how many changed. */
  async update(table: string, where: Where, set: Row) {
    const { params, bind } = binder();
    const sql = `update ${table} set ${setSql(this.stored(table, set), bind)}${whereSql(this.stored(table, where), bind)}`;
    return Number(this.db.prepare(sql).run(...params).changes);
  }

  /** Deletes the matching rows; answers how many went. */
  async delete(table: string, where: Where) {
    const { params, bind } = binder();
    return Number(
      this.db.prepare(`delete from ${table}${whereSql(this.stored(table, where), bind)}`).run(...params).changes,
    );
  }

  /** Whether the table exists in this storage. */
  async exists(table: string) {
    return Boolean(this.db.prepare("select 1 from sqlite_master where type = 'table' and name = ?").get(table));
  }

  /** The control plane's remote client, or null to keep it in its own SQLite file. */
  controlRemote() {
    return null;
  }

  /** Releases what the driver holds. */
  async close() {
    this.db.close();
  }

  /** Plain values of a filter or a set, converted for their columns; bounds and lists pass through. */
  private stored(table: string, values: Row) {
    const types = columnsOf(table);
    return Object.fromEntries(Object.entries(values).map(([c, v]) => [c, isTest(v) ? v : toStored(types[c], v)]));
  }

  /** Runs `work` as one transaction, so a batch is all written or none of it. */
  private transaction(work: () => void) {
    this.db.exec('begin immediate');
    try {
      work();
      this.db.exec('commit');
    } catch (e) {
      this.db.exec('rollback');
      throw e;
    }
  }
}
