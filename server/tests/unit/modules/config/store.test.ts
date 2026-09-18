/**
 * Unit tests for the settings store's write-behind: it writes the fallback file
 * when there is no database, never overlaps writes, and stays dirty when a write fails.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, rmSync } from 'node:fs';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { store, state, flush, writeField, dropField, seal, unseal } =
  await import('../../../../src/modules/config/store.ts');
const { STORE, readStoreFile } = await import('../../../../src/modules/config/repository.ts');

describe('settings store', () => {
  beforeEach(() => {
    store.clear();
    state.dirty = false;
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

  it('seals a value for one owner that only that owner opens', () => {
    const sealed = seal('owner-1', { token: 'x' });
    assert.deepEqual(unseal('owner-1', sealed), { token: 'x' });
    assert.throws(() => unseal('owner-2', sealed));
  });
});
