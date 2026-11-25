/**
 * Reading another browser's SQLite files (cookies, history, bookmarks) by the system's
 * bundled SQLite, from a copy taken with its write-ahead log: a running browser keeps its
 * newest rows there and holds a lock on the original.
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { copyIfPresent } from './copy.ts';

/** The rows `sql` answers from a copy of `file`, or none when there is no such file. */
export function sqliteRows<T>(file: string, sql: string): T[] {
  if (!fs.existsSync(file)) return [];
  const copy = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'oya-db-')), 'db.sqlite');
  try {
    for (const suffix of ['', '-wal']) copyDatabaseFile(file + suffix, copy + suffix);
    return sqliteJson<T>(copy, sql);
  } finally {
    fs.rmSync(path.dirname(copy), { recursive: true, force: true });
  }
}

/** The rows `sql` answers from `file`, read-only. */
function sqliteJson<T>(file: string, sql: string): T[] {
  const db = new DatabaseSync(file, { readOnly: true, allowExtension: false });
  try {
    const statement = db.prepare(sql);
    statement.setReadBigInts(true);
    return statement.all().map(numericRow) as T[];
  } finally {
    db.close();
  }
}

/** Chromium timestamps exceed the safe integer range; retain the former JSON reader's numeric contract. */
function numericRow(row: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(
    Object.entries(row).map(([key, value]) => [key, typeof value === 'bigint' ? Number(value) : value]),
  );
}

/** A source profile cannot redirect an offline read through a file symlink. */
function copyDatabaseFile(source: string, destination: string): void {
  if (fs.existsSync(source) && fs.lstatSync(source).isSymbolicLink()) throw Error('Unsupported linked database file.');
  copyIfPresent(source, destination);
}
