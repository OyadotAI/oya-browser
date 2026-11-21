/**
 * Unit tests for the signed-in user's project routes (/auth/projects): without
 * Supabase auth configured every route answers that auth is unavailable, and a
 * request without a token is refused first. The handlers behind the auth wall
 * are covered through http/projects and the service tests. Console access is
 * rate limited per person and project, so a looping tab cannot flood the store.
 */
import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';
import { tokenWithAal } from '../../support/mfa.ts';
import { Status } from '../../../../src/platform/http-status.ts';

ownDataDir('oya-control-membership-');
const { projectAccountRouter, AUTO_NAME } = await import('../../../../src/modules/control/membership.ts');
const { callRoute } = await import('../../support/agent.ts');
const { sessionUsers } = await import('../../../../src/modules/auth/middleware.ts');
const { DEFAULT_CONSOLE_ACCESS_PER_MIN } = await import('../../../../src/platform/constants.ts');

describe('projectAccountRouter', () => {
  it('refuses a request without a token', async () => {
    const res = await callRoute(projectAccountRouter, { url: '/' });
    assert.deepEqual([res.status, res.body], [401, { error: 'Missing token' }]);
  });

  it('answers 503 while account auth is not configured', async () => {
    const res = await callRoute(projectAccountRouter, { method: 'POST', url: '/prj_x/key', key: 'jwt' });
    assert.deepEqual([res.status, res.body], [503, { error: 'Auth not configured' }]);
  });
});

describe('AUTO_NAME', () => {
  it('matches only the name a project gets before anyone names it', () => {
    assert.equal(AUTO_NAME.test('Project a1b2c3'), true);
    assert.equal(AUTO_NAME.test('Project Acme'), false);
  });
});

describe('POST /auth/projects/:id/access', () => {
  const real = { ...sessionUsers };
  after(() => Object.assign(sessionUsers, real));
  /** Signs every request in as one person, whose projects the control store does not know. */
  const signedIn = (id: string) =>
    Object.assign(sessionUsers, {
      configured: () => true,
      getUser: async () => ({ data: { user: { id, email: 'ada@example.com', factors: [] } }, error: null }),
    });
  /** Asks for console access to `project`. */
  const access = (project: string) =>
    callRoute(projectAccountRouter, { method: 'POST', url: `/${project}/access`, key: tokenWithAal('aal1') });

  it('answers 429 once a person has asked for one project too often in a minute, and only for that project', async () => {
    signedIn(`u-limit-${Date.now()}`);
    for (let i = 0; i < DEFAULT_CONSOLE_ACCESS_PER_MIN; i++)
      assert.notEqual((await access('prj_looping')).status, Status.TOO_MANY_REQUESTS);
    const refused = await access('prj_looping');
    assert.equal(refused.status, Status.TOO_MANY_REQUESTS);
    assert.notEqual((await access('prj_other')).status, Status.TOO_MANY_REQUESTS);
  });
});
