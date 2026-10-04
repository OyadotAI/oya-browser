/**
 * Unit tests for the SQLite driver's own schema care: a file written before a
 * column was added to schema.ts gains that column on open, keeping its rows.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { SqliteConnection } from '../../../../src/platform/storage/sqlite.ts';

describe('SqliteConnection', () => {
  it('adds the schema columns an older file lacks, keeping its rows', async () => {
    const path = join(mkdtempSync(join(tmpdir(), 'oya-sqlite-')), 'storage.sqlite');
    const old = new DatabaseSync(path);
    old.exec('create table audit_log (id integer primary key autoincrement, action text)');
    old.exec("insert into audit_log (action) values ('before')");
    old.close();
    const db = new SqliteConnection(path);
    await db.upsert('audit_log', [{ action: 'after', credential_id: 'c-1' }]);
    const rows = await db.select('audit_log', {}, { order: ['id', 'asc'] });
    await db.close();
    assert.deepEqual(
      rows.map((r) => [r.action, r.credential_id]),
      [
        ['before', null],
        ['after', 'c-1'],
      ],
    );
  });
});
