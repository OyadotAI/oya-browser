/**
 * Unit tests for loading settings at startup from storage, taking in the
 * pre-storage key-settings.json once, and waiting on pending writes at shutdown.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, rmSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { restore, drain } = await import('../../../../src/modules/config/persistence.ts');
const { store, state, seal } = await import('../../../../src/modules/config/store.ts');
const { LEGACY_STORE, writeRows } = await import('../../../../src/modules/config/repository.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

/** Writes the legacy settings file as the old server did: [owner, fields] entries. */
function writeLegacy(content: string) {
  mkdirSync(dirname(LEGACY_STORE), { recursive: true });
  writeFileSync(LEGACY_STORE, content);
}

/** One owner's stored fields. */
async function storedFields(owner: string) {
  const rows = await getConnection().select('key_settings', { owner });
  return Object.fromEntries(rows.map((r) => [r.key, r.value]));
}

describe('settings persistence', () => {
  beforeEach(async () => {
    store.clear();
    state.dirty = false;
    await getConnection().delete('key_settings', {});
    for (const path of [LEGACY_STORE, `${LEGACY_STORE}.imported`]) rmSync(path, { force: true });
  });

  it('restores every owner’s fields from storage', async () => {
    await writeRows(['owner-1'], [{ owner: 'owner-1', key: 'chat_model', value: 'm' }]);
    await restore();
    assert.deepEqual(store.get('owner-1'), { chat_model: 'm' });
  });

  it('treats empty storage as a fresh install', async () => {
    await restore();
    assert.equal(store.size, 0);
  });

  it('takes in the legacy file once and sets it aside as .imported', async () => {
    writeLegacy(JSON.stringify([['owner-1', { chat_model: 'legacy' }]]));
    await restore();
    assert.deepEqual(await storedFields('owner-1'), { chat_model: 'legacy' });
    assert.equal(existsSync(LEGACY_STORE), false);
    assert.equal(existsSync(`${LEGACY_STORE}.imported`), true);
  });

  it('keeps a stored field over the legacy file’s stale copy of it', async () => {
    await writeRows(['owner-1'], [{ owner: 'owner-1', key: 'chat_model', value: 'stored' }]);
    writeLegacy(JSON.stringify([['owner-1', { chat_model: 'legacy' }]]));
    await restore();
    assert.deepEqual(store.get('owner-1'), { chat_model: 'stored' });
  });

  it('fails the start on a legacy file it cannot parse, and leaves the file in place', async () => {
    writeLegacy('{not json');
    await assert.rejects(restore(), SyntaxError);
    assert.equal(existsSync(LEGACY_STORE), true);
  });

  it('cleans saved playbooks as they load, keeping the original as a backup', async () => {
    const pb = {
      name: 'p',
      steps: [
        { action: 'type', el: { tag: 'input', type: 'hidden' } },
        { action: 'click', el: { text: 'Go' } },
      ],
    };
    await writeRows(['owner-1'], [{ owner: 'owner-1', key: '_playbook:p', value: seal('owner-1', pb) }]);
    await restore();
    assert.ok(store.get('owner-1')['_playbook-backup:v1:p']);
    assert.ok((await storedFields('owner-1'))['_playbook-backup:v1:p'], 'the cleaned store was written out');
  });

  it('waits for pending writes on drain', async () => {
    store.set('owner-1', { a: '1' });
    state.dirty = true;
    await drain();
    assert.deepEqual(await storedFields('owner-1'), { a: '1' });
  });
});
