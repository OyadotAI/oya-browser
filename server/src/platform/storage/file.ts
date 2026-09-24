/**
 * The file driver: each table is a JSON array in data/storage/<table>.json,
 * held in memory and rewritten whole, through a temporary file and a rename,
 * on every change. For development and single-box installs that want files an
 * operator can read; one process only.
 */
import { readFileSync, writeFileSync, renameSync, mkdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { TABLES, rowFromStored } from './schema.ts';
import { FILE_INDENT, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE } from './constants.ts';
import type { Connection, Row, SelectOptions, Where } from './connection.ts';

/** Whether one row passes one filter entry, with the meanings sql.ts gives them in SQL. */
function passes(value: unknown, test: any) {
  if (test === null) return value === null || value === undefined;
  if (test && typeof test === 'object' && 'gte' in test) return value !== null && value >= test.gte;
  if (test && typeof test === 'object' && 'notIn' in test) return !test.notIn.includes(value);
  return value === test;
}

/** Whether a row passes every entry of a filter. */
const matches = (row: Row, where: Where) => Object.entries(where).every(([c, test]) => passes(row[c], test));

/** Rows sorted and cut as a select asks. */
function shaped(rows: Row[], { order, limit }: SelectOptions) {
  const sign = order?.[1] === 'desc' ? -1 : 1;
  const sorted = order
    ? [...rows].sort((a, b) => (a[order[0]] > b[order[0]] ? sign : a[order[0]] < b[order[0]] ? -sign : 0))
    : rows;
  return limit ? sorted.slice(0, limit) : sorted;
}

/** A row's key, as one string for matching. */
const keyOf = (table: string, row: Row) => JSON.stringify(TABLES[table].key.map((c) => row[c]));

/** JSON files behind the storage contract. */
export class FileConnection implements Connection {
  /** Which driver this is. */
  readonly kind = 'file' as const;
  /** The directory the table files live in. */
  declare readonly dir: string;
  /** Tables read so far, by name. */
  declare private tables: Map<string, Row[]>;
  /** A driver on the storage at `dir`. */
  constructor(dir: string) {
    this.dir = dir;
    this.tables = new Map();
  }

  /** Rows of `table` matching `where`, sorted and cut as asked. */
  async select(table: string, where: Where = {}, options: SelectOptions = {}) {
    return shaped(
      this.rows(table).filter((row) => matches(row, where)),
      options,
    ).map((row) => rowFromStored(table, structuredClone(row)));
  }

  /** Inserts rows; with `update`, replaces a row whose key is already stored. */
  async upsert(table: string, rows: Row[], { update = false } = {}) {
    const stored = this.rows(table);
    for (const row of rows) this.put(table, stored, row, update);
    this.save(table);
  }

  /** Sets `set` on the matching rows; answers how many changed. */
  async update(table: string, where: Where, set: Row) {
    const hits = this.rows(table).filter((row) => matches(row, where));
    for (const row of hits) Object.assign(row, set);
    if (hits.length) this.save(table);
    return hits.length;
  }

  /** Deletes the matching rows; answers how many went. */
  async delete(table: string, where: Where) {
    const rows = this.rows(table);
    const kept = rows.filter((row) => !matches(row, where));
    this.tables.set(table, kept);
    if (kept.length !== rows.length) this.save(table);
    return rows.length - kept.length;
  }

  /** Whether the table exists in this storage. */
  async exists(table: string) {
    return Object.hasOwn(TABLES, table) || existsSync(this.path(table));
  }

  /** The control plane's remote client, or null to keep it in its own SQLite file. */
  controlRemote() {
    return null;
  }

  /** Releases what the driver holds. */
  async close() {
    this.tables.clear();
  }

  /** Inserts one row, numbering a serial key, or replaces the stored one when `update` is set. */
  private put(table: string, stored: Row[], row: Row, update: boolean) {
    const serial = Object.entries(TABLES[table].columns).find(([, type]) => type === 'serial')?.[0];
    if (serial && row[serial] == null) return void stored.push({ ...row, [serial]: nextId(stored, serial) });
    const at = stored.findIndex((r) => keyOf(table, r) === keyOf(table, row));
    if (at < 0) stored.push({ ...row });
    else if (update) stored[at] = { ...stored[at], ...row };
  }

  /** A table's rows, read from its file the first time. */
  private rows(table: string) {
    if (!this.tables.has(table))
      this.tables.set(table, existsSync(this.path(table)) ? JSON.parse(readFileSync(this.path(table), 'utf8')) : []);
    return this.tables.get(table);
  }

  /** Writes a table whole, through a temporary file so a crash never leaves half of it. */
  private save(table: string) {
    mkdirSync(this.dir, { recursive: true, mode: PRIVATE_DIR_MODE });
    const temporary = `${this.path(table)}.tmp`;
    writeFileSync(temporary, JSON.stringify(this.tables.get(table), null, FILE_INDENT), { mode: PRIVATE_FILE_MODE });
    renameSync(temporary, this.path(table));
  }

  /** Where a table's file is. */
  private path(table: string) {
    return join(this.dir, `${table}.json`);
  }
}

/** The next number for a serial column. */
const nextId = (rows: Row[], column: string) => rows.reduce((max, r) => Math.max(max, Number(r[column]) || 0), 0) + 1;
