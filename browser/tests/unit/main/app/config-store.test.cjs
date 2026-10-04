/**
 * Unit tests for ConfigStore: defaults, the saved file, environment
 * overrides, a broken file, the owner-only write, and the API key sealed
 * with a stand-in for Electron's safeStorage.
 */
const { describe, it, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { ConfigStore } = require('../../../../main/app/config-store.cjs');

/** A stand-in for safeStorage: reversible, and visibly not the key. */
const fakeSafe = (backend = 'keychain') => ({
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => backend,
  encryptString: (text) => Buffer.from(`sealed:${text}`),
  decryptString: (buffer) => buffer.toString().replace(/^sealed:/, ''),
});

describe('ConfigStore', () => {
  let dir;
  beforeEach(() => (dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-config-'))));
  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  it('starts from the defaults when nothing is saved', () => {
    const store = new ConfigStore({ dir: () => dir, env: {}, platform: 'linux' });
    assert.deepEqual(store.load(), {
      serverUrl: 'wss://oyabrowser.com/ws',
      apiKey: '',
      browserName: 'Oya Browser on Linux',
      activeProfileId: null,
      mirroredFrom: '',
    });
  });

  it('keeps what was saved, and the environment wins over it', () => {
    fs.writeFileSync(path.join(dir, 'config.json'), JSON.stringify({ apiKey: 'saved', browserName: 'Mine' }));
    const env = {
      OYA_API_KEY: ' first , second',
      OYA_SERVER_URL: 'wss://s.test/ws',
      OYA_PERSONA: 'p',
      OYA_PROVIDER: 'oya-cloud',
    };
    const values = new ConfigStore({ dir: () => dir, env }).load();
    assert.equal(values.apiKey, 'first');
    assert.equal(values.browserName, 'Mine');
    assert.deepEqual([values.serverUrl, values.persona, values.provider], ['wss://s.test/ws', 'p', 'oya-cloud']);
  });

  it('stays logged out after a log out, whatever key the environment carries', () => {
    fs.writeFileSync(path.join(dir, 'config.json'), JSON.stringify({ apiKey: '', signedOut: true }));
    assert.equal(new ConfigStore({ dir: () => dir, env: { OYA_API_KEY: 'env-key' } }).load().apiKey, '');
  });

  it('keeps a key and server entered in the app over the environment', () => {
    const saved = { apiKey: 'typed', serverUrl: 'wss://mine.test/ws', keyFromApp: true };
    fs.writeFileSync(path.join(dir, 'config.json'), JSON.stringify(saved));
    const env = { OYA_API_KEY: 'a2a_other_product', OYA_SERVER_URL: 'wss://other.test/ws', OYA_PERSONA: 'p' };
    const values = new ConfigStore({ dir: () => dir, env }).load();
    assert.deepEqual([values.apiKey, values.serverUrl, values.persona], ['typed', 'wss://mine.test/ws', 'p']);
  });

  it('ignores a broken file', () => {
    fs.writeFileSync(path.join(dir, 'config.json'), '{nope');
    assert.equal(new ConfigStore({ dir: () => dir, env: {} }).load().apiKey, '');
  });

  it('writes owner-only, tightening a file written before', () => {
    const file = path.join(dir, 'config.json');
    fs.writeFileSync(file, '{}', { mode: 0o644 });
    const store = new ConfigStore({ dir: () => dir, env: {} });
    store.load();
    store.merge({ apiKey: 'k' });
    store.save();
    assert.equal(fs.statSync(file).mode & 0o777, 0o600);
    assert.equal(JSON.parse(fs.readFileSync(file, 'utf8')).apiKey, 'k');
  });

  it('does not throw when it cannot write', () => {
    const store = new ConfigStore({ dir: () => path.join(dir, 'missing'), env: {} });
    store.load();
    store.save();
  });

  describe('the API key on disk', () => {
    const file = () => path.join(dir, 'config.json');
    const onDisk = () => JSON.parse(fs.readFileSync(file(), 'utf8'));
    const save = (safe, key) => {
      const store = new ConfigStore({ dir: () => dir, env: {}, safe });
      store.load();
      store.merge({ apiKey: key });
      store.save();
    };

    it('is sealed with the keychain, never written in plain text', () => {
      save(fakeSafe(), 'oya_secret');
      assert.equal(onDisk().apiKey, '');
      assert.doesNotMatch(fs.readFileSync(file(), 'utf8'), /oya_secret"/);
      assert.equal(new ConfigStore({ dir: () => dir, env: {}, safe: fakeSafe() }).load().apiKey, 'oya_secret');
    });

    it('seals a key saved in plain text by an older version when it is read', () => {
      fs.writeFileSync(file(), JSON.stringify({ apiKey: 'oya_old' }));
      const values = new ConfigStore({ dir: () => dir, env: {}, safe: fakeSafe() }).load();
      assert.equal(values.apiKey, 'oya_old');
      assert.equal(onDisk().apiKey, '');
      assert.ok(onDisk().apiKeySealed);
    });

    it('does not seal a key the environment supplied', () => {
      new ConfigStore({ dir: () => dir, env: { OYA_API_KEY: 'from-env' }, safe: fakeSafe() }).load();
      assert.equal(fs.existsSync(file()), false);
    });

    it('stays in the owner-only file when the keychain only obscures (Linux basic_text)', () => {
      save(fakeSafe('basic_text'), 'oya_plain');
      assert.equal(onDisk().apiKey, 'oya_plain');
      assert.equal(onDisk().apiKeySealed, undefined);
    });

    it('stays in the owner-only file outside Electron', () => {
      save(null, 'oya_plain');
      assert.equal(onDisk().apiKey, 'oya_plain');
    });

    it('reads as empty when this keychain cannot open it', () => {
      save(fakeSafe(), 'oya_secret');
      const broken = { ...fakeSafe(), decryptString: () => assert.fail('not this keychain') };
      assert.equal(new ConfigStore({ dir: () => dir, env: {}, safe: broken }).load().apiKey, '');
    });

    it('keeps a key the keychain could not open this launch, so a later launch still has it', () => {
      save(fakeSafe(), 'oya_secret');
      const locked = { ...fakeSafe(), decryptString: () => assert.fail('keychain locked') };
      const store = new ConfigStore({ dir: () => dir, env: {}, safe: locked });
      store.load();
      store.merge({ browserName: 'Desk' });
      store.save();
      assert.equal(new ConfigStore({ dir: () => dir, env: {}, safe: fakeSafe() }).load().apiKey, 'oya_secret');
    });

    it('reads as empty when the keychain has gone away', () => {
      save(fakeSafe(), 'oya_secret');
      const gone = { isEncryptionAvailable: () => false };
      assert.equal(new ConfigStore({ dir: () => dir, env: {}, safe: gone }).load().apiKey, '');
    });
  });
});
