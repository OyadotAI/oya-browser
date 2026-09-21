/**
 * Unit tests for loading settings at startup from the fallback file, and for
 * waiting on pending writes at shutdown. (Without a database configured the
 * database branch is not reachable from a unit test.)
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { restore, drain } = await import('../../../../src/modules/config/persistence.ts');
const { store, state, seal } = await import('../../../../src/modules/config/store.ts');
const { STORE, readStoreFile, writeStoreFile } = await import('../../../../src/modules/config/repository.ts');

describe('settings persistence', () => {
  beforeEach(() => {
    store.clear();
    state.dirty = false;
    rmSync(STORE, { force: true, recursive: true });
  });

  it('restores every owner’s fields from the fallback file', async () => {
    await writeStoreFile([['owner-1', { chat_model: 'm' }]]);
    await restore();
    assert.deepEqual(store.get('owner-1'), { chat_model: 'm' });
  });

  it('treats a missing file as a fresh install', async () => {
    const error = mock.method(console, 'error', () => {});
    await restore();
    assert.equal(store.size, 0);
    assert.equal(error.mock.callCount(), 0);
    error.mock.restore();
  });

  it('logs a file it cannot parse and starts empty', async () => {
    mkdirSync(dirname(STORE), { recursive: true });
    writeFileSync(STORE, '{not json');
    const error = mock.method(console, 'error', () => {});
    await restore();
    assert.equal(store.size, 0);
    assert.match(error.mock.calls[0].arguments[0], /restore failed/);
    error.mock.restore();
  });

  it('cleans saved playbooks as they load, keeping the original as a backup', async () => {
    const pb = {
      name: 'p',
      steps: [
        { action: 'type', el: { tag: 'input', type: 'hidden' } },
        { action: 'click', el: { text: 'Go' } },
      ],
    };
    await writeStoreFile([['owner-1', { '_playbook:p': seal('owner-1', pb) }]]);
    await restore();
    const row = store.get('owner-1');
    assert.ok(row['_playbook-backup:v1:p']);
    const saved = await readStoreFile();
    assert.ok(saved[0][1]['_playbook-backup:v1:p'], 'the cleaned store was written out');
  });

  it('waits for pending writes on drain', async () => {
    store.set('owner-1', { a: '1' });
    state.dirty = true;
    await drain();
    assert.deepEqual(await readStoreFile(), [['owner-1', { a: '1' }]]);
  });
});
