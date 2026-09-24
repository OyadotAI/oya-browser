/**
 * The storage contract, run against both local drivers: SQLite and JSON files
 * must answer every call the same way, because modules never know which one
 * they have. Each driver gets its own scratch directory.
 */
import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SqliteConnection } from '../../../../src/platform/storage/sqlite.ts';
import { FileConnection } from '../../../../src/platform/storage/file.ts';
import type { Connection } from '../../../../src/platform/storage/index.ts';

/** How to open each local driver in a directory. */
const DRIVERS: Record<string, (dir: string) => Connection> = {
  sqlite: (dir) => new SqliteConnection(join(dir, 'storage.sqlite')),
  file: (dir) => new FileConnection(join(dir, 'storage')),
};

/** A persona row with every column type in it. */
const persona = (id: string, extra: object = {}) => ({
  id,
  owner: 'o',
  name: id,
  seed: 4294967295,
  prefs: { platform: 'Win32', list: [1, 2] },
  max_concurrent: null,
  is_default: true,
  created_at: '2026-09-01T00:00:00.000Z',
  ...extra,
});

for (const [kind, open] of Object.entries(DRIVERS)) {
  describe(`${kind} driver`, () => {
    let dir: string;
    let db: Connection;
    before(() => {
      dir = mkdtempSync(join(tmpdir(), `oya-storage-${kind}-`));
      db = open(dir);
    });
    after(async () => {
      await db.close();
      rmSync(dir, { recursive: true, force: true });
    });

    it('reads back what it stored, each column as its JavaScript type', async () => {
      await db.upsert('personas', [persona('types')]);
      const [row] = await db.select('personas', { id: 'types' });
      assert.equal(row.seed, 4294967295);
      assert.deepEqual(row.prefs, { platform: 'Win32', list: [1, 2] });
      assert.equal(row.is_default, true);
      assert.equal(row.max_concurrent, null);
      assert.equal(row.created_at, '2026-09-01T00:00:00.000Z');
    });

    it('leaves a stored row alone on insert, and replaces it on upsert with update', async () => {
      await db.upsert('personas', [persona('keep', { name: 'first' })]);
      await db.upsert('personas', [persona('keep', { name: 'second' })]);
      assert.equal((await db.select('personas', { id: 'keep' }))[0].name, 'first');
      await db.upsert('personas', [persona('keep', { name: 'third' })], { update: true });
      assert.equal((await db.select('personas', { id: 'keep' }))[0].name, 'third');
    });

    it('matches on equality, null, a lower bound and an exclusion list', async () => {
      await db.upsert('api_keys', [
        { key_hash: 'a', user_id: 'u1', created_at: '2026-01-01T00:00:00.000Z' },
        { key_hash: 'b', user_id: null, created_at: '2026-03-01T00:00:00.000Z' },
        { key_hash: 'c', user_id: 'u1', created_at: '2026-05-01T00:00:00.000Z' },
      ]);
      const hashes = async (where) => (await db.select('api_keys', where)).map((r) => r.key_hash).sort();
      assert.deepEqual(await hashes({ user_id: 'u1' }), ['a', 'c']);
      assert.deepEqual(await hashes({ user_id: null }), ['b']);
      assert.deepEqual(await hashes({ created_at: { gte: '2026-02-01T00:00:00.000Z' } }), ['b', 'c']);
      assert.deepEqual(await hashes({ user_id: 'u1', key_hash: { notIn: ['a'] } }), ['c']);
      assert.deepEqual(await hashes({ user_id: 'u1', key_hash: { notIn: [] } }), ['a', 'c']);
    });

    it('sorts and cuts a select', async () => {
      const rows = await db.select('api_keys', {}, { order: ['created_at', 'desc'], limit: 2 });
      assert.deepEqual(
        rows.map((r) => r.key_hash),
        ['c', 'b'],
      );
    });

    it('updates only matching rows and says how many', async () => {
      assert.equal(await db.update('api_keys', { key_hash: 'b', user_id: null }, { user_id: 'u2' }), 1);
      assert.equal(await db.update('api_keys', { key_hash: 'b', user_id: null }, { user_id: 'u3' }), 0);
      assert.equal((await db.select('api_keys', { key_hash: 'b' }))[0].user_id, 'u2');
    });

    it('deletes only matching rows and says how many', async () => {
      assert.equal(await db.delete('api_keys', { key_hash: 'a', user_id: 'someone-else' }), 0);
      assert.equal(await db.delete('api_keys', { key_hash: 'a', user_id: 'u1' }), 1);
      assert.deepEqual(await db.select('api_keys', { key_hash: 'a' }), []);
    });

    it('numbers a serial column itself', async () => {
      await db.upsert('audit_log', [{ ts: '2026-09-01T00:00:00.000Z', action: 'one', outcome: 'ok' }]);
      await db.upsert('audit_log', [{ ts: '2026-09-02T00:00:00.000Z', action: 'two', outcome: 'ok' }]);
      const ids = (await db.select('audit_log', {}, { order: ['ts', 'asc'] })).map((r) => r.id);
      assert.equal(ids.length, 2);
      assert.ok(ids[1] > ids[0]);
    });

    it('knows its own tables, and not a legacy table it never had', async () => {
      assert.equal(await db.exists('personas'), true);
      assert.equal(await db.exists('browsers'), false);
    });

    it('keeps the control plane in its own SQLite file', () => {
      assert.equal(db.controlRemote(), null);
    });

    it('still holds its data when opened again', async () => {
      await db.close();
      db = open(dir);
      assert.equal((await db.select('personas', { id: 'types' }))[0].seed, 4294967295);
    });
  });
}

describe('file driver', () => {
  it('hands out copies, so a caller changing a row does not change what is stored', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'oya-storage-copy-'));
    const db = new FileConnection(dir);
    await db.upsert('personas', [persona('copy')]);
    (await db.select('personas'))[0].prefs.platform = 'changed';
    assert.equal((await db.select('personas'))[0].prefs.platform, 'Win32');
    rmSync(dir, { recursive: true, force: true });
  });
});
