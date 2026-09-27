/**
 * Unit tests for the admin routes: every one is behind sign-in and the admin
 * check, and each hands its request to the service.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
import { FakeResponse } from '../../support/auth.ts';

ownDataDir('oya-admin-routes-');
const { router } = await import('../../../../src/modules/admin/routes.ts');
const { adminOnly } = await import('../../../../src/modules/admin/access.ts');
const { userAuthMiddleware } = await import('../../../../src/modules/auth/service.ts');
const license = await import('../../../../src/platform/license/index.ts');

/** Each route's path, method and handlers. */
const routes = router.stack.filter((l: any) => l.route).map((l: any) => l.route);

/** Calls a route's last handler, past its guards. */
async function call(method: string, path: string, req: any) {
  const route = routes.find((r) => r.path === path && r.methods[method]);
  const res = new FakeResponse();
  await route.stack.at(-1).handle({ user: { email: 'mk@getoya.ai' }, body: {}, query: {}, params: {}, ...req }, res);
  return res;
}

describe('admin routes', () => {
  afterEach(() => {
    license.setModuleForTests(null);
    delete process.env.OYA_LICENSE_SIGNING_KEY;
  });

  it('puts every route behind sign-in and the admin check', () => {
    assert.ok(routes.length >= 5);
    for (const r of routes) {
      const handlers = r.stack.map((s) => s.handle);
      assert.equal(handlers[0], userAuthMiddleware, r.path);
      assert.equal(handlers[1], adminOnly, r.path);
    }
  });

  it('answers the overview and the license list', async () => {
    assert.ok((await call('get', '/admin/overview', {})).body.accounts);
    assert.deepEqual((await call('get', '/admin/licenses', {})).body, { licenses: [] });
  });

  it('issues a license with 201, naming the admin who did', async () => {
    license.setModuleForTests({
      start() {},
      admit: () => ({ ok: true, message: '' }),
      mint: () => ({
        license: { id: 'L9', licensee: 'A', maxConcurrent: 9, expiresAt: '2030-01-01T00:00:00.000Z' },
        token: 'K',
      }),
    });
    process.env.OYA_LICENSE_SIGNING_KEY = 'k';
    const res = await call('post', '/admin/licenses', { body: { licensee: 'A' } });
    assert.equal(res.statusCode, 201);
    assert.equal(res.body.key, 'K');
    assert.equal((await call('post', '/admin/licenses/:id/revoke', { params: { id: 'L9' } })).body.id, 'L9');
  });

  it('looks up a person by the email asked for', async () => {
    await assert.rejects(call('get', '/admin/users', { query: { email: 'nobody@example.com' } }), { status: 404 });
  });
});
