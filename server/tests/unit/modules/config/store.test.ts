/**
 * Unit tests for the settings store's write-behind: it writes the fallback file
 * when there is no database, never overlaps writes, and stays dirty when a write fails.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, rmSync } from 'node:fs';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { store, state, changed, flush, writeField, dropField, seal, unseal, markChanged, pendingWrite } =
  await import('../../../../src/modules/config/store.ts');
const { STORE, readStoreFile, writeDb } = await import('../../../../src/modules/config/repository.ts');

/** A Supabase stand-in for key_settings that records each call, in order. */
function fakeDb() {
  const calls: any[] = [];
  const query = (op: any) => {
    const q: any = { eq: (c, v) => ((op.eq = [c, v]), q), not: (c, o, v) => ((op.not = [c, o, v]), q) };
    q.then = (resolve) => resolve({ error: null });
    return q;
  };
  const table = {
    upsert: async (rows, options) => (
      calls.push({ upsert: rows.map((r) => [r.owner, r.key]), options }),
      { error: null }
    ),
    delete: () => {
      const op: any = { delete: true };
      calls.push(op);
      return query(op);
    },
  };
  return { client: { from: () => table } as any, calls };
}

describe('settings store', () => {
  beforeEach(() => {
    store.clear();
    state.dirty = false;
    changed.clear();
    rmSync(STORE, { recursive: true, force: true });
  });

  it('writes a field to the fallback file', async () => {
    await writeField('owner-1', 'chat_model', 'm');
    assert.deepEqual(await readStoreFile(), [['owner-1', { chat_model: 'm' }]]);
    assert.equal(state.dirty, false);
  });

  it('removes a field and writes the row without it', async () => {
    await writeField('owner-1', 'a', '1');
    await writeField('owner-1', 'b', '2');
    await dropField('owner-1', 'a');
    assert.deepEqual(await readStoreFile(), [['owner-1', { b: '2' }]]);
  });

  it('skips the write when nothing changed', async () => {
    await flush();
    await assert.rejects(readStoreFile(), { code: 'ENOENT' });
  });

  it('stays dirty for the next flush when the file cannot be written', async () => {
    mkdirSync(STORE, { recursive: true }); // a directory where the file should go
    const error = mock.method(console, 'error', () => {});
    await writeField('owner-1', 'a', '1');
    assert.equal(state.dirty, true);
    assert.match(error.mock.calls[0].arguments[0], /file fallback failed/);
    rmSync(STORE, { recursive: true });
    await flush();
    assert.equal(state.dirty, false);
    error.mock.restore();
  });

  it('runs overlapping flushes one after another, ending with the latest state', async () => {
    store.set('owner-1', { a: '1' });
    state.dirty = true;
    const first = flush();
    await writeField('owner-1', 'a', '2');
    await first;
    assert.deepEqual(await readStoreFile(), [['owner-1', { a: '2' }]]);
  });

  it('writes only the owner that changed, not every tenant', () => {
    store.set('owner-1', { a: '1' });
    store.set('owner-2', { b: '2', c: '3' });
    markChanged('owner-2');
    assert.deepEqual(pendingWrite(), {
      owners: ['owner-2'],
      rows: [
        { owner: 'owner-2', key: 'b', value: '2' },
        { owner: 'owner-2', key: 'c', value: '3' },
      ],
    });
  });

  it('writes every owner when a change was not attributed to one', () => {
    store.set('owner-1', { a: '1' });
    store.set('owner-2', { b: '2' });
    state.dirty = true;
    assert.deepEqual(pendingWrite().owners, ['owner-1', 'owner-2']);
  });

  it('upserts an owner’s rows before deleting only the keys it no longer has', async () => {
    const { client, calls } = fakeDb();
    await writeDb(['owner-2'], [{ owner: 'owner-2', key: 'say "hi"', value: 'x' }], client);
    assert.deepEqual(calls[0].upsert, [['owner-2', 'say "hi"']]);
    assert.deepEqual(calls[1], { delete: true, eq: ['owner', 'owner-2'], not: ['key', 'in', '("say \\"hi\\"")'] });
  });

  it('clears an owner whose every field was removed', async () => {
    const { client, calls } = fakeDb();
    await writeDb(['owner-3'], [], client);
    assert.deepEqual(calls, [{ delete: true, eq: ['owner', 'owner-3'] }]);
  });

  it('seals a value for one owner that only that owner opens', () => {
    const sealed = seal('owner-1', { token: 'x' });
    assert.deepEqual(unseal('owner-1', sealed), { token: 'x' });
    assert.throws(() => unseal('owner-2', sealed));
  });
});
