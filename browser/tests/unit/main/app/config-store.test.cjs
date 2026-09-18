/**
 * Unit tests for ConfigStore: defaults, the saved file, environment
 * overrides, a broken file, and the owner-only write.
 */
const { describe, it, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { ConfigStore } = require('../../../../main/app/config-store.cjs');

describe('ConfigStore', () => {
  let dir;
  beforeEach(() => (dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-config-'))));
  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  it('starts from the defaults when nothing is saved', () => {
    const store = new ConfigStore({ dir: () => dir, env: {}, platform: 'linux' });
    assert.deepEqual(store.load(), {
      serverUrl: 'ws://localhost:3100/ws',
      apiKey: '',
      browserName: 'Oya Browser linux',
      activeProfileId: null,
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
});
