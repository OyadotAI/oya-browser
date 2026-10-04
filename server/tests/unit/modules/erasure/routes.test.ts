/**
 * Unit tests for the account route: DELETE /auth/me needs a signed-in person
 * before erasure is asked anything, and other methods fall through to the
 * auth routes that own them.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough } from '../../support/auth.ts';

ownDataDir('oya-erasure-routes-');
const { accountRoutes } = await import('../../../../src/modules/erasure/index.ts');

/** An erasure stand-in that records who asked to be deleted. */
const fakeErasure = (asked: string[]) =>
  ({
    deleteAccount: async (id: string) => {
      asked.push(id);
      return { ok: true, projects: 0 };
    },
  }) as any;

describe('account routes', () => {
  it('refuses to delete an account without a signed-in person', async () => {
    const asked = [];
    const res = await routeThrough(accountRoutes(fakeErasure(asked)), fakeRequest({ method: 'DELETE' }));
    assert.equal(res.statusCode, 401);
    assert.deepEqual(asked, []);
  });

  it('leaves GET /auth/me to the auth routes', async () => {
    const res = await routeThrough(accountRoutes(fakeErasure([])), fakeRequest({ method: 'GET' }));
    assert.equal(res.statusCode, 404);
  });
});
