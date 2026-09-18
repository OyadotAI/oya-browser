/**
 * Unit tests for per-user API keys without a database: registering a key
 * claims its project for the user, owners are resolved and cached, keys are
 * minted with projects, and the database-only operations answer as they must.
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
      message: 'Key belongs to another account',
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

describe('without a database', () => {
  it('lists no keys', async () => {
    assert.deepEqual(await apiKeys.listApiKeys('user-a'), []);
  });

  it('refuses to delete a key with 409', async () => {
    await assert.rejects(apiKeys.deleteApiKey('digest', 'user-a'), {
      status: Status.CONFLICT,
      message: 'Accounts need Supabase',
    });
  });

  it('records no last use, and does not fail', async () => {
    assert.equal(await apiKeys.touchApiKey('anything'), undefined);
  });
});
