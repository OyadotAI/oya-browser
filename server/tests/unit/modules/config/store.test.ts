/**
 * Unit tests for the settings store's write-behind: it writes the key_settings
 * table through the configured storage, never overlaps writes, touches only the
 * owners that changed, and keeps a failed write queued for the next flush.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { store, state, changed, flush, writeField, dropField, seal, unseal, markChanged, pendingWrite } =
  await import('../../../../src/modules/config/store.ts');
const { writeRows } = await import('../../../../src/modules/config/repository.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

/** What key_settings holds, as owner -> { field: value }. */
async function stored() {
  const out: Record<string, Record<string, string>> = {};
  for (const row of await getConnection().select('key_settings'))
    out[row.owner] = { ...out[row.owner], [row.key]: row.value };
  return out;
}

describe('settings store', () => {
  beforeEach(async () => {
    store.clear();
    state.dirty = false;
    changed.clear();
    await getConnection().delete('key_settings', {});
  });

  it('writes a field to storage', async () => {
    await writeField('owner-1', 'chat_model', 'm');
    assert.deepEqual(await stored(), { 'owner-1': { chat_model: 'm' } });
    assert.equal(state.dirty, false);
  });

  it('removes a field and deletes its row', async () => {
    await writeField('owner-1', 'a', '1');
    await writeField('owner-1', 'b', '2');
    await dropField('owner-1', 'a');
    assert.deepEqual(await stored(), { 'owner-1': { b: '2' } });
  });

  it('skips the write when nothing changed', async () => {
    const upsert = mock.method(getConnection(), 'upsert');
    await flush();
    assert.equal(upsert.mock.callCount(), 0);
    upsert.mock.restore();
  });

  it('keeps a failed write’s owners queued and says so, and the next flush writes them', async () => {
    const error = mock.method(console, 'error', () => {});
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    await writeField('owner-1', 'a', '1');
    assert.equal(state.dirty, true);
    assert.deepEqual([...changed], ['owner-1']);
    assert.match(error.mock.calls[0].arguments[0], /write failed \(storage down\)/);
    upsert.mock.restore();
    await flush();
    assert.equal(state.dirty, false);
    assert.deepEqual(await stored(), { 'owner-1': { a: '1' } });
    error.mock.restore();
  });

  it('runs overlapping flushes one after another, ending with the latest state', async () => {
    store.set('owner-1', { a: '1' });
    state.dirty = true;
    const first = flush();
    await writeField('owner-1', 'a', '2');
    await first;
    assert.deepEqual(await stored(), { 'owner-1': { a: '2' } });
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

  it('replaces an owner’s rows, keeping a quoted key and leaving other owners alone', async () => {
    await writeRows(
      ['owner-1', 'owner-2'],
      [
        { owner: 'owner-1', key: 'keep', value: '1' },
        { owner: 'owner-2', key: 'old', value: '2' },
      ],
    );
    await writeRows(['owner-2'], [{ owner: 'owner-2', key: 'say "hi"', value: 'x' }]);
    assert.deepEqual(await stored(), { 'owner-1': { keep: '1' }, 'owner-2': { 'say "hi"': 'x' } });
  });

  it('clears an owner whose every field was removed', async () => {
    await writeRows(['owner-3'], [{ owner: 'owner-3', key: 'a', value: '1' }]);
    await writeRows(['owner-3'], []);
    assert.deepEqual(await stored(), {});
  });

  it('seals a value for one owner that only that owner opens', () => {
    const sealed = seal('owner-1', { token: 'x' });
    assert.deepEqual(unseal('owner-1', sealed), { token: 'x' });
    assert.throws(() => unseal('owner-2', sealed));
  });
});
