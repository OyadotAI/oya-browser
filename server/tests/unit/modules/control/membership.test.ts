/**
 * Unit tests for the signed-in user's project routes (/auth/projects): without
 * Supabase auth configured every route answers that auth is unavailable, and a
 * request without a token is refused first. The handlers behind the auth wall
 * are covered through http/projects and the service tests.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-control-membership-');
const { projectAccountRouter, AUTO_NAME } = await import('../../../../src/modules/control/membership.ts');
const { callRoute } = await import('../../support/agent.ts');

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
