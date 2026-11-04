/** Bundled SQLite reads active WAL snapshots without a system executable and cleans failed copies. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { sqliteRows } from '../../../../src/main/mirror/sqlite.ts';
it('reads committed WAL rows while the source connection remains open', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-wal-test-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const file = path.join(root, 'History');
  const db = new DatabaseSync(file);
  try {
    db.exec(
      "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE urls(url TEXT); INSERT INTO urls VALUES ('https://example.com/');",
    );
    assert(fs.existsSync(`${file}-wal`));
    assert.deepEqual(
      sqliteRows<{ /** Fixture address. */ url: string }>(file, 'SELECT url FROM urls').map((row) => row.url),
      ['https://example.com/'],
    );
  } finally {
    db.close();
  }
});
it('cleans scratch copies when a linked database file is refused', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-db-cleanup-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const file = path.join(root, 'History');
  fs.writeFileSync(file, 'synthetic');
  fs.symlinkSync(file, `${file}-wal`);
  const original = fs.mkdtempSync;
  const created: string[] = [];
  t.mock.method(fs, 'mkdtempSync', (prefix: string) => {
    const dir = original(prefix);
    created.push(dir);
    return dir;
  });
  assert.throws(() => sqliteRows(file, 'SELECT 1'), /linked database/);
  assert(created.every((dir) => !fs.existsSync(dir)));
});
