/**
 * Unit tests for the agent signup routes on the tests' own storage: a
 * challenge is handed out, a signup is refused for a missing email or an
 * unsolved puzzle, and a caller address gets three signups a day, each minting
 * a stored, unclaimed key.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough } from '../../support/auth.ts';

ownDataDir('oya-agent-routes-');
const { router } = await import('../../../../src/modules/auth/agent-routes.ts');
const { solves } = await import('../../../../src/modules/auth/agents.ts');
const keys = await import('../../../../src/modules/auth/keys.ts');
const { isUnclaimedAgentKey } = await import('../../../../src/modules/auth/api-keys.ts');

/** Sends one request through the agent router; a thrown HttpError rejects. */
const send = (method: string, path: string, body: any = {}, headers: any = {}) =>
  routeThrough(router, fakeRequest({ method, path, body, headers }));

/** A freshly issued, solved challenge. */
async function solved() {
  const { challenge } = (await send('GET', '/auth/agent/challenge')).body;
  let n = 0;
  while (!solves(challenge, String(n))) n++;
  return { challenge, nonce: String(n) };
}

/** Rejects with `status`. */
const rejectsWith = (promise: Promise<unknown>, status: number) =>
  assert.rejects(promise, (err: any) => err.status === status);

describe('agent signup', () => {
  it('hands out a challenge', async () => {
    const res = await send('GET', '/auth/agent/challenge');
    assert.equal(typeof res.body.challenge, 'string');
    assert.equal(res.body.difficulty, 5);
  });

  it('requires the email of the person the agent works for', async () => {
    await rejectsWith(send('POST', '/auth/agent/signup', { ...(await solved()) }), Status.BAD_REQUEST);
  });

  it('refuses an unsolved puzzle', async () => {
    const { challenge } = (await send('GET', '/auth/agent/challenge')).body;
    const nonce = ['a', 'b', 'c'].find((n) => !solves(challenge, n));
    const body = { email: 'owner@example.com', challenge, nonce };
    await rejectsWith(send('POST', '/auth/agent/signup', body), Status.BAD_REQUEST);
  });

  it('allows an address three signups a day, each minting a stored unclaimed key, then answers 429', async () => {
    const headers = { 'x-forwarded-for': '198.51.100.7' };
    for (let i = 0; i < 3; i++) {
      const body = { email: 'owner@example.com', ...(await solved()) };
      const { api_key } = (await send('POST', '/auth/agent/signup', body, headers)).body;
      assert.equal(keys.validateApiKey(api_key), true);
      assert.equal(await isUnclaimedAgentKey(api_key), true);
    }
    const body = { email: 'owner@example.com', ...(await solved()) };
    await rejectsWith(send('POST', '/auth/agent/signup', body, headers), Status.TOO_MANY_REQUESTS);
  });
});
