/**
 * Unit tests for registering a browser provider for a key: the config is
 * validated, the vendor must be known and have a credential, names are unique
 * per key, the CDP URL is vetted, and a supplied vendor key is saved.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir('oya-provider-registration-');
const { registerProvider } = await import('../../../../src/modules/gateway/provider-registration.ts');
const { pool } = await import('../../../../src/modules/gateway/routing.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const KEY = 'registration-key';
const OWNER = fingerprint(KEY);

describe('registerProvider', () => {
  let savedAnchor: string | undefined;
  beforeEach(() => {
    savedAnchor = process.env.ANCHOR_API_KEY;
    delete process.env.ANCHOR_API_KEY;
  });
  afterEach(() => {
    restoreEnv('ANCHOR_API_KEY', savedAnchor);
    for (const p of pool.visible(OWNER)) if (p.owner === OWNER) pool.remove(OWNER, p.name);
  });

  it("registers a CDP provider under the key's owner", async () => {
    const p = await registerProvider(KEY, OWNER, { name: 'home', wsUrl: 'ws://8.8.8.8:9222/devtools' });
    assert.deepEqual([p.name, p.owner, p.wsUrl], ['home', OWNER, 'ws://8.8.8.8:9222/devtools']);
    assert.equal(pool.get(OWNER, 'home'), p);
  });

  it('ignores an owner in the body: the caller’s own is used', async () => {
    const p = await registerProvider(KEY, OWNER, { name: 'home', wsUrl: 'ws://8.8.8.8:9222', owner: null });
    assert.equal(p.owner, OWNER);
  });

  it('refuses an invalid config with a 400', async () => {
    await assert.rejects(registerProvider(KEY, OWNER, { name: 'x', wsUrl: 'http://8.8.8.8' }), { status: 400 });
    await assert.rejects(registerProvider(KEY, OWNER, null), { status: 400 });
  });

  it('refuses an unknown vendor with a 400', async () => {
    await assert.rejects(registerProvider(KEY, OWNER, { name: 'x', type: 'nosuchvendor' }), {
      status: 400,
      message: 'Unknown browser provider.',
    });
  });

  it('refuses a known vendor without a credential with a 409', async () => {
    await assert.rejects(registerProvider(KEY, OWNER, { name: 'x', type: 'anchor' }), {
      status: 409,
      message: 'Add an API key for this provider, or save one in Settings → Browsers.',
    });
  });

  it('accepts a vendor key sent with the provider, and saves it for the key', async () => {
    const p = await registerProvider(KEY, OWNER, { name: 'hosted', type: 'anchor', apiKey: ' sk-anchor ' });
    assert.equal(p.type, 'anchor');
    assert.equal(keyConfig.envFor(KEY).ANCHOR_API_KEY, 'sk-anchor');
  });

  it('refuses a name the key already uses, with a 409', async () => {
    await registerProvider(KEY, OWNER, { name: 'home', wsUrl: 'ws://8.8.8.8:9222' });
    await assert.rejects(registerProvider(KEY, OWNER, { name: 'home', wsUrl: 'ws://8.8.4.4:9222' }), {
      status: 409,
      message: 'A provider with this name already exists. Choose another name.',
    });
  });

  it('refuses a CDP URL on a private address unless the host opts in', async () => {
    const saved = process.env.OYA_ALLOW_PRIVATE_TARGETS;
    delete process.env.OYA_ALLOW_PRIVATE_TARGETS;
    await assert.rejects(registerProvider(KEY, OWNER, { name: 'lan', wsUrl: 'ws://10.0.0.5:9222' }), {
      status: 400,
      message: /^wsUrl/,
    });
    restoreEnv('OYA_ALLOW_PRIVATE_TARGETS', saved);
  });

  it('never allows the cloud metadata address', async () => {
    await assert.rejects(registerProvider(KEY, OWNER, { name: 'meta', wsUrl: 'ws://169.254.169.254/' }), {
      status: 400,
    });
  });
});
