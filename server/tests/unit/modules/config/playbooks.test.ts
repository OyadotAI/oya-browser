/**
 * Unit tests for playbooks in the sealed settings store: saving, reading,
 * listing and deleting by key, and the startup cleanup that keeps backups.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const books = await import('../../../../src/modules/config/playbooks.ts');
const { store, state, seal } = await import('../../../../src/modules/config/store.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const KEY = 'config-playbooks-key';
const OWNER = fingerprint(KEY);
const HIDDEN_STEP = { action: 'type', text: 'x', el: { tag: 'input', type: 'hidden' } };

describe('stored playbooks', () => {
  beforeEach(() => {
    store.clear();
    state.dirty = false;
  });

  it('saves a playbook sealed and reads it back with its name', async () => {
    await books.savePlaybook(KEY, 'login', { steps: [{ action: 'click' }] });
    assert.deepEqual(books.getPlaybook(KEY, 'login'), { steps: [{ action: 'click' }], name: 'login' });
    assert.ok(!JSON.stringify(store.get(OWNER)).includes('click'));
  });

  it('reads nothing for a name that was never saved, or under another key', async () => {
    await books.savePlaybook(KEY, 'login', { steps: [] });
    assert.equal(books.getPlaybook(KEY, 'other'), null);
    assert.equal(books.getPlaybook('another-key', 'login'), null);
  });

  it('reads a playbook that no longer unseals as absent, and leaves it out of the list', async () => {
    await books.savePlaybook(KEY, 'good', { steps: [] });
    store.get(OWNER)['_playbook:bad'] = seal('someone-else', { steps: [] });
    assert.equal(books.getPlaybook(KEY, 'bad'), null);
    assert.deepEqual(
      books.listPlaybooks(KEY).map((p) => p.name),
      ['good'],
    );
  });

  it('lists only playbook rows, not other settings', async () => {
    await books.savePlaybook(KEY, 'a', { steps: [] });
    store.get(OWNER).chat_model = 'm';
    assert.deepEqual(
      books.listPlaybooks(KEY).map((p) => p.name),
      ['a'],
    );
  });

  it('deletes a playbook', async () => {
    await books.savePlaybook(KEY, 'a', { steps: [] });
    await books.deletePlaybook(KEY, 'a');
    assert.deepEqual(books.listPlaybooks(KEY), []);
  });

  describe('cleanupPlaybooks', () => {
    it('cleans a noisy playbook and keeps the sealed original as a backup', async () => {
      await books.savePlaybook(KEY, 'noisy', { steps: [HIDDEN_STEP, { action: 'click', el: { text: 'Go' } }] });
      const original = store.get(OWNER)['_playbook:noisy'];
      assert.deepEqual(await books.cleanupPlaybooks(), { updated: 1, unreadable: 0 });
      assert.equal(store.get(OWNER)['_playbook-backup:v1:noisy'], original);
      assert.equal(books.getPlaybook(KEY, 'noisy').steps.length, 1);
    });

    it('keeps the first backup when cleaning again', async () => {
      await books.savePlaybook(KEY, 'noisy', { steps: [HIDDEN_STEP] });
      const original = store.get(OWNER)['_playbook:noisy'];
      await books.cleanupPlaybooks();
      await books.savePlaybook(KEY, 'noisy', { steps: [HIDDEN_STEP, HIDDEN_STEP] });
      await books.cleanupPlaybooks();
      assert.equal(store.get(OWNER)['_playbook-backup:v1:noisy'], original);
    });

    it('leaves a clean playbook alone and counts one that will not unseal', async () => {
      await books.savePlaybook(KEY, 'clean', { steps: [{ action: 'click', el: { text: 'Go' } }] });
      store.get(OWNER)['_playbook:bad'] = seal('someone-else', { steps: [] });
      assert.deepEqual(await books.cleanupPlaybooks(), { updated: 0, unreadable: 1 });
      assert.equal(store.get(OWNER)['_playbook-backup:v1:clean'], undefined);
    });
  });
});
