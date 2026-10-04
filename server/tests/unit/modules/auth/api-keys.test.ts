/**
 * Unit tests for per-user API keys on the tests' own storage: registering a
 * key claims its project for the user, owners are resolved and cached, keys are
 * minted with projects, and a user lists and deletes only their own keys;
 * deleting one ends its live browser sockets and gateway sessions.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-api-keys-');
/** A key listed in API_KEYS, as a deployment's own integration key is; set before the modules read it. */
const ENV_KEY = 'env-integration-key'.padEnd(32, 'e');
process.env.API_KEYS = [process.env.API_KEYS, ENV_KEY].filter(Boolean).join(',');
const apiKeys = await import('../../../../src/modules/auth/api-keys.ts');
const keys = await import('../../../../src/modules/auth/keys.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { registry } = await import('../../../../src/modules/browsers/registry.ts');
const { sessions } = await import('../../../../src/modules/gateway/service.ts');
// Loaded for the revocation listeners they register.
await import('../../../../src/modules/browsers/connection/admission.ts');
const { FakeSocket } = await import('../../support/fakes.ts');

/** The stored project row for a key. */
const projectOf = (key: string) => control().store.get('project', projectId(key));

describe('getKeyOwner', () => {
  it('finds no owner for a missing or unknown key', async () => {
    assert.equal(await apiKeys.getKeyOwner(''), null);
    assert.equal(await apiKeys.getKeyOwner(keys.generateKey()), null);
  });
});

describe('registeredOwner', () => {
  it('names who registered a key listed in API_KEYS, which getKeyOwner leaves ownerless on purpose', async () => {
    await apiKeys.registerApiKey(ENV_KEY, 'user-env', 'Integration');
    assert.equal(await apiKeys.getKeyOwner(ENV_KEY), null, 'an env key keeps off its owner’s model settings');
    assert.equal(await apiKeys.registeredOwner(ENV_KEY), 'user-env');
  });

  it('finds nobody for a key nobody registered', async () => {
    assert.equal(await apiKeys.registeredOwner(keys.generateKey()), null);
  });
});

describe('registerApiKey', () => {
  it('makes the key valid, owned by the user, and claims its project', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-a', 'Checkout bot');
    assert.equal(keys.validateApiKey(key), true);
    assert.equal(await apiKeys.getKeyOwner(key), 'user-a');
    const project = await projectOf(key);
    assert.equal(project.ownerUser, 'user-a');
    assert.equal(project.name, 'Checkout bot', 'a new project takes its key’s label');
  });

  it('never stores the key in the clear on its project', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-a', 'x');
    assert.ok(!JSON.stringify(await projectOf(key)).includes(key));
  });

  it('cuts a long label to the project-name limit', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-a', 'n'.repeat(150));
    assert.equal((await projectOf(key)).name.length, 100);
  });

  it('keeps the default project name without a label', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-a');
    assert.match((await projectOf(key)).name, /^Project [0-9a-f]{6}$/);
  });

  it('opens an unowned project for a key with no user', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, null, 'label');
    assert.equal(keys.validateApiKey(key), true);
    const project = await projectOf(key);
    assert.equal(project.ownerUser, undefined);
    assert.match(project.name, /^Project /, 'only a user’s key names its project');
  });

  it('refuses to hand a project owned by one account to another', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-a', 'mine');
    await assert.rejects(apiKeys.registerApiKey(key, 'user-b', 'theirs'), {
      status: Status.FORBIDDEN,
      message: 'Key cannot be imported',
    });
    const project = await projectOf(key);
    assert.deepEqual([project.ownerUser, project.name], ['user-a', 'mine']);
    assert.equal(await apiKeys.getKeyOwner(key), 'user-a', 'a refused import does not re-own the key');
  });

  it('lets the owner register the same key again', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-a', 'first');
    await apiKeys.registerApiKey(key, 'user-a', 'second');
    assert.equal((await projectOf(key)).ownerUser, 'user-a');
  });
});

describe('provisionKeys', () => {
  it('mints the requested number of distinct valid keys, each with a project', async () => {
    const minted = await apiKeys.provisionKeys(3);
    assert.equal(new Set(minted).size, 3);
    for (const key of minted) {
      assert.equal(keys.validateApiKey(key), true);
      assert.ok(await projectOf(key));
      assert.equal(await apiKeys.getKeyOwner(key), null, 'a minted key belongs to nobody yet');
    }
  });

  it('mints nothing for zero', async () => {
    assert.deepEqual(await apiKeys.provisionKeys(0), []);
  });
});

describe('listing and deleting keys', () => {
  it('lists a user’s keys by digest and prefix, never the key', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-list', 'listed');
    const [listed] = await apiKeys.listApiKeys('user-list');
    assert.deepEqual([listed.id, listed.prefix, listed.label], [keys.keyDigest(key), key.slice(0, 8), 'listed']);
    assert.ok(!JSON.stringify(listed).includes(key));
  });

  it('lists nothing for a user with no keys', async () => {
    assert.deepEqual(await apiKeys.listApiKeys('user-none'), []);
  });

  it('deletes a user’s own key, after which it no longer validates', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-del');
    await apiKeys.deleteApiKey(keys.keyDigest(key), 'user-del');
    assert.equal(keys.validateApiKey(key), false);
    assert.deepEqual(await apiKeys.listApiKeys('user-del'), []);
  });

  it('answers 404 for another user’s key, and leaves it', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-own');
    await assert.rejects(apiKeys.deleteApiKey(keys.keyDigest(key), 'user-other'), { status: Status.NOT_FOUND });
    assert.equal((await apiKeys.listApiKeys('user-own')).length, 1);
  });

  it('records a last use, and does not fail for an unknown key', async () => {
    assert.equal(await apiKeys.touchApiKey('anything'), undefined);
  });
});

describe('key expiry', () => {
  /** A fixed clock, so expiry times are exact. */
  const NOW = Date.parse('2026-01-01T00:00:00.000Z');

  it('never expires a key created without expiresInDays when there is no cap', () => {
    assert.equal(apiKeys.keyExpiry(undefined, null, NOW), null);
  });

  it('expires a key the requested number of days from now', () => {
    assert.equal(apiKeys.keyExpiry(30, null, NOW), '2026-01-31T00:00:00.000Z');
  });

  for (const bad of [0, 3651, 1.5, '30', -1])
    it(`refuses expiresInDays ${JSON.stringify(bad)} with 400`, () => {
      assert.throws(() => apiKeys.keyExpiry(bad, null, NOW), { status: Status.BAD_REQUEST });
    });

  it('caps a requested lifetime at the operator maximum', () => {
    assert.equal(apiKeys.keyExpiry(365, 90, NOW), apiKeys.keyExpiry(90, null, NOW));
  });

  it('gives a never-expiring request the operator maximum', () => {
    assert.equal(apiKeys.keyExpiry(undefined, 90, NOW), apiKeys.keyExpiry(90, null, NOW));
  });

  it('keeps a shorter lifetime than the operator maximum', () => {
    assert.equal(apiKeys.keyExpiry(30, 90, NOW), apiKeys.keyExpiry(30, null, NOW));
  });

  it('lists a key’s expiry, and null for a key that never expires', async () => {
    const [dated, never] = [keys.generateKey(), keys.generateKey()];
    await apiKeys.registerApiKey(dated, 'user-expiry', 'dated', '2030-01-01T00:00:00.000Z');
    await apiKeys.registerApiKey(never, 'user-expiry-never', 'never');
    assert.equal((await apiKeys.listApiKeys('user-expiry'))[0].expires_at, '2030-01-01T00:00:00.000Z');
    assert.equal((await apiKeys.listApiKeys('user-expiry-never'))[0].expires_at, null);
  });

  it('keeps a key’s expiry when its owner re-imports it', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-reimport', 'x', '2030-01-01T00:00:00.000Z');
    await apiKeys.registerApiKey(key, 'user-reimport', 'x', null);
    assert.equal((await apiKeys.listApiKeys('user-reimport'))[0].expires_at, '2030-01-01T00:00:00.000Z');
  });
});

describe('revoking a key', () => {
  it('drops the key’s browsers and ends its gateway sessions, and leaves other keys’', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-rev');
    const [mine, theirs] = [new FakeSocket(), new FakeSocket()];
    registry.add('rev-mine', { ws: mine, apiKey: key, name: 'Mine' } as any);
    registry.add('rev-theirs', { ws: theirs, apiKey: 'other-key', name: 'Theirs' } as any);
    const destroy = mock.fn(async () => {});
    sessions.set('rev-session', { apiKey: key, destroy } as any);
    await apiKeys.deleteApiKey(keys.keyDigest(key), 'user-rev');
    sessions.delete('rev-session');
    assert.deepEqual([registry.get('rev-mine'), mine.closed?.reason], [undefined, 'API key revoked']);
    assert.ok(registry.get('rev-theirs'));
    assert.deepEqual(destroy.mock.calls[0].arguments, ['API key revoked']);
    registry.remove('rev-theirs');
  });

  it('tells every listener even when one of them fails', async () => {
    const key = keys.generateKey();
    await apiKeys.registerApiKey(key, 'user-rev2');
    mock.method(console, 'error', () => {});
    const heard: string[] = [];
    apiKeys.onKeyRevoked(() => {
      throw new Error('boom');
    });
    apiKeys.onKeyRevoked((digest) => heard.push(digest));
    await apiKeys.deleteApiKey(keys.keyDigest(key), 'user-rev2');
    assert.deepEqual(heard, [keys.keyDigest(key)]);
    mock.restoreAll();
  });
});
