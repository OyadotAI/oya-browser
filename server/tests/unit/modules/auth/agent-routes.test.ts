/**
 * Unit tests for the agent signup routes without Supabase: a challenge is
 * handed out, a signup is refused for a missing email or an unsolved puzzle,
 * a caller address gets three signups a day, and with no database the key
 * cannot be minted.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';
import { fakeRequest, routeThrough } from '../../support/auth.ts';

ownDataDir('oya-agent-routes-');
const { router } = await import('../../../../src/modules/auth/agent-routes.ts');
const { solves } = await import('../../../../src/modules/auth/agents.ts');

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

  it('allows an address three signups a day, then answers 429', async () => {
    const headers = { 'x-forwarded-for': '198.51.100.7' };
    for (let i = 0; i < 3; i++) {
      const body = { email: 'owner@example.com', ...(await solved()) };
      // No Supabase here, so each admitted signup stops at minting the key.
      await rejectsWith(send('POST', '/auth/agent/signup', body, headers), Status.UNAVAILABLE);
    }
    const body = { email: 'owner@example.com', ...(await solved()) };
    await rejectsWith(send('POST', '/auth/agent/signup', body, headers), Status.TOO_MANY_REQUESTS);
  });
});
