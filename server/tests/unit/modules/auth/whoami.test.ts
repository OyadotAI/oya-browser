/**
 * Unit tests for GET /auth/whoami: a key answers for its own owner with the
 * project and plan, a member's credential for that member alone, a key nobody
 * owns for no one, and a share link is refused.
 */
import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough } from '../../support/auth.ts';

ownDataDir('oya-whoami-');
const savedKeys = process.env.API_KEYS;
process.env.API_KEYS = 'env-admin-key';
after(() => restoreEnv('API_KEYS', savedKeys));
const { whoamiRoutes } = await import('../../../../src/modules/auth/whoami.ts');
const { registerApiKey } = await import('../../../../src/modules/auth/api-keys.ts');
const { generateKey } = await import('../../../../src/modules/auth/keys.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const { control, projectId } = await import('../../../../src/modules/control/service.ts');
const { issue } = await import('../../../../src/modules/control/service/credentials.ts');

/** The router, with every account on the Pro plan. */
const router = whoamiRoutes(async () => 'pro');

/** Asks whoami with `token` as the bearer. */
const ask = (token: string) =>
  routeThrough(router, fakeRequest({ path: '/auth/whoami', headers: { authorization: `Bearer ${token}` } }));

/** A person with a profile, and a key registered to them. */
async function personWithKey(id: string, email: string) {
  await getConnection().upsert('profiles', [{ id, email, display_name: `Name ${id}`, created_at: '2026-01-01' }]);
  const key = generateKey();
  await registerApiKey(key, id, 'Desktop');
  return key;
}

describe('GET /auth/whoami', () => {
  it("names a stored key's owner, its project and plan", async () => {
    const key = await personWithKey('u-ada', 'ada@example.com');
    const res = await ask(key);
    assert.equal(res.statusCode, Status.OK);
    const project = { id: projectId(key), name: (await control().project(key)).name };
    const expected = { email: 'ada@example.com', name: 'Name u-ada', plan: 'pro', role: 'administrator', project };
    assert.deepEqual(res.body, expected);
  });

  it('names nobody for a key no account owns', async () => {
    const res = await ask('env-admin-key');
    assert.deepEqual([res.body.email, res.body.name, res.body.plan], [null, null, null]);
    assert.equal(res.body.project.id, projectId('env-admin-key'));
  });

  it("names a member's credential's member, never the project owner", async () => {
    const key = await personWithKey('u-owner', 'owner@example.com');
    await getConnection().upsert('profiles', [{ id: 'u-bo', email: 'bo@example.com', created_at: '2026-01-01' }]);
    const project = projectId(key);
    const membership = { id: `${project}:u-bo`, project, userId: 'u-bo', role: 'operator' };
    await control().store.transact(async (tx) => tx.put('membership', membership.id, membership));
    const grant = { role: 'operator', label: 'member', expiresAt: null, memberUser: 'u-bo', sessionId: null };
    const res = await ask((await issue(control().store, key, grant)).token);
    assert.deepEqual([res.body.email, res.body.role], ['bo@example.com', 'operator']);
  });

  it('names nobody for a project credential no member holds', async () => {
    const key = await personWithKey('u-cy', 'cy@example.com');
    const { token } = await control().credential(key, { role: 'viewer' });
    assert.equal((await ask(token)).body.email, null);
  });

  it('refuses a share link', async () => {
    const key = await personWithKey('u-di', 'di@example.com');
    const grant = { role: 'operator', label: 'share', expiresAt: null, memberUser: null, sessionId: 'sess-1' };
    const res = await ask((await issue(control().store, key, grant)).token);
    assert.equal(res.statusCode, Status.FORBIDDEN);
  });

  it('refuses a missing key', async () => {
    assert.equal((await ask('')).statusCode, Status.UNAUTHORIZED);
  });
});
