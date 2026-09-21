/**
 * Unit tests for scripts/draft-store.cjs: encrypted drafts on disk, the
 * keychain-protected key, and recovery from damaged files.
 */
const { describe, it, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { DraftStore } = require('../../../scripts/draft-store.cjs');

/** A safeStorage stand-in that "encrypts" by base64, so a test can see what it was given. */
const safe = (overrides = {}) => ({
  isEncryptionAvailable: () => true,
  encryptString: (value) => Buffer.from(value),
  decryptString: (value) => value.toString(),
  ...overrides,
});

describe('DraftStore', () => {
  let dir;
  beforeEach(() => (dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-drafts-'))));
  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  it('saves a paused, normalized draft that loads back unchanged', () => {
    const store = new DraftStore(dir, safe());
    const saved = store.save({
      id: 'd1',
      name: 'n',
      phase: 'recording',
      steps: [{ action: 'navigate', url: 'https://x.test' }],
    });
    assert.equal(saved.phase, 'paused');
    assert.deepEqual(store.load('d1'), saved);
  });

  it('never writes the draft in the clear', () => {
    const store = new DraftStore(dir, safe());
    store.save({ id: 'd1', steps: [{ action: 'navigate', url: 'https://secret.test' }] });
    assert.ok(!fs.readFileSync(store.file('d1')).includes('secret.test'));
    assert.equal(fs.statSync(store.file('d1')).mode & 0o777, 0o600);
  });

  it('reuses its key across instances', () => {
    new DraftStore(dir, safe()).save({ id: 'd1' });
    assert.equal(new DraftStore(dir, safe()).load('d1').id, 'd1');
  });

  it('refuses a tampered file', () => {
    const store = new DraftStore(dir, safe());
    store.save({ id: 'd1' });
    const data = fs.readFileSync(store.file('d1'));
    data[data.length - 1] ^= 1;
    fs.writeFileSync(store.file('d1'), data);
    assert.throws(() => store.load('d1'));
  });

  it('refuses to save without a secure keychain', () => {
    const plain = new DraftStore(dir, safe({ getSelectedStorageBackend: () => 'basic_text' }));
    assert.throws(() => plain.save({ id: 'd1' }), /Secure local storage is unavailable/);
    const none = new DraftStore(dir, safe({ isEncryptionAvailable: () => false }));
    assert.throws(() => none.save({ id: 'd1' }), /Secure local storage/);
  });

  it('refuses an id that could leave the folder', () => {
    assert.throws(() => new DraftStore(dir, safe()).file('../x'), /Invalid draft ID/);
  });

  it('lists drafts newest first, with unreadable ones marked for recovery', () => {
    const store = new DraftStore(dir, safe());
    store.save({ id: 'old', name: 'Old', updatedAt: 1 });
    store.save({ id: 'new', name: 'New', updatedAt: 2, steps: [{}] });
    fs.writeFileSync(path.join(dir, 'broken.enc'), 'junk');
    assert.deepEqual(store.list(), [
      { id: 'new', name: 'New', updatedAt: 2, steps: 1 },
      { id: 'old', name: 'Old', updatedAt: 1, steps: 0 },
      { id: 'broken', name: 'Draft needs recovery', error: true },
    ]);
  });

  it('reports a draft’s size and removes it, missing or not', () => {
    const store = new DraftStore(dir, safe());
    store.save({ id: 'd1' });
    assert.ok(store.size('d1') > 0);
    store.remove('d1');
    store.remove('d1');
    assert.deepEqual(store.list(), []);
  });
});
