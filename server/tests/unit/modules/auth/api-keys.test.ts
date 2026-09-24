/**
 * Unit tests for per-user API keys on the tests' own storage: registering a
 * key claims its project for the user, owners are resolved and cached, keys are
 * minted with projects, and a user lists and deletes only their own keys.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-api-keys-');
const apiKeys = await import('../../../../src/modules/auth/api-keys.ts');
const keys = await import('../../../../src/modules/auth/keys.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');

/** The stored project row for a key. */
const projectOf = (key: string) => control().store.get('project', projectId(key));

describe('getKeyOwner', () => {
  it('finds no owner for a missing or unknown key', async () => {
    assert.equal(await apiKeys.getKeyOwner(''), null);
    assert.equal(await apiKeys.getKeyOwner(keys.generateKey()), null);
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
