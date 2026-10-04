/**
 * Unit tests for erasure's storage: a project's rows go from every table keyed
 * by its owner or id (its personas' sealed records included) and nobody
 * else's do; a person's keys and profile go, their subscription and the audit
 * log stay.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-erasure-rows-');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const rows = await import('../../../../src/modules/erasure/repository.ts');

const T = new Date().toISOString();

/** One row per table for the project's owner `mine`, and the same for `theirs`. */
async function seed(owner: string, project: string, persona: string, user: string) {
  const db = getConnection();
  await db.upsert('personas', [{ id: persona, owner, name: 'p', seed: 1, created_at: T }]);
  await db.upsert('persona_logins', [{ id: persona, value: 's', updated_at: T }]);
  await db.upsert('persona_credentials', [{ id: `${persona}|example.com`, value: 's', updated_at: T }]);
  await db.upsert('mfa_factors', [{ id: persona, value: 's', updated_at: T }]);
  await db.upsert('routines', [{ owner, id: 'r', value: 's', version: 1, updated_at: T }]);
  await db.upsert('key_settings', [{ owner, key: 'model', value: 'x', updated_at: T }]);
  await db.upsert('usage', [{ api_key: owner, hour: T, commands: 1, updated_at: T }]);
  await db.upsert('api_keys', [{ key_hash: `h-${owner}`, key_prefix: 'oya', project, user_id: user, created_at: T }]);
  await db.upsert('profiles', [{ id: user, email: `${user}@x`, created_at: T }]);
  await db.upsert('subscriptions', [{ user_id: user, plan: 'free', updated_at: T }]);
}

/** How many rows each table holds. */
async function counts() {
  const tables = ['personas', 'persona_logins', 'persona_credentials', 'mfa_factors', 'routines'];
  const more = ['key_settings', 'usage', 'api_keys', 'profiles', 'subscriptions'];
  const out = {};
  for (const t of [...tables, ...more]) out[t] = (await getConnection().select(t)).length;
  return out;
}

beforeEach(async () => {
  for (const t of ['personas', 'persona_logins', 'persona_credentials', 'mfa_factors', 'routines', 'key_settings'])
    await getConnection().delete(t, {});
  for (const t of ['usage', 'api_keys', 'profiles', 'subscriptions']) await getConnection().delete(t, {});
  await seed('mine0000mine0000', 'prj_mine', 'per_mine', 'u-mine');
  await seed('them0000them0000', 'prj_them', 'per_them', 'u-them');
});

describe('erasure rows', () => {
  it("deletes a project's rows from every owner- or project-keyed table, and only its", async () => {
    await rows.deleteProjectRows('mine0000mine0000', 'prj_mine');
    const left = await counts();
    for (const t of ['personas', 'persona_logins', 'persona_credentials', 'mfa_factors', 'routines'])
      assert.equal(left[t], 1, t);
    for (const t of ['key_settings', 'usage', 'api_keys']) assert.equal(left[t], 1, t);
    assert.equal((await getConnection().select('personas'))[0].id, 'per_them');
    assert.equal((await getConnection().select('api_keys'))[0].project, 'prj_them');
  });

  it('runs again harmlessly when the project is already gone', async () => {
    await rows.deleteProjectRows('mine0000mine0000', 'prj_mine');
    await rows.deleteProjectRows('mine0000mine0000', 'prj_mine');
    assert.equal((await counts())['personas'], 1);
  });

  it("deletes a person's keys and profile, keeping their subscription", async () => {
    await rows.deleteUserRows('u-mine');
    assert.deepEqual(
      (await getConnection().select('profiles')).map((r) => r.id),
      ['u-them'],
    );
    assert.equal((await getConnection().select('api_keys', { user_id: 'u-mine' })).length, 0);
    assert.equal((await rows.subscriptionOf('u-mine')).plan, 'free');
  });

  it('answers null for a person with no subscription', async () => {
    assert.equal(await rows.subscriptionOf('nobody'), null);
  });

  it('has no sign-in to delete on a server without Supabase', async () => {
    await rows.deleteSignIn('u-mine');
  });
});
