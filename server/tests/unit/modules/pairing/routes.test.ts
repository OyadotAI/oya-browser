/**
 * Unit tests for the pairing routes: an authenticated caller gets a code, and
 * the desktop app redeems it without a credential, rate limited by address.
 */
import { describe, it, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { router } = await import('../../../../src/modules/pairing/routes.ts');
const pairing = await import('../../../../src/modules/pairing/service.ts');
const { LIMITS } = await import('../../../../src/platform/limits.ts');
const { allowKey, callRoute } = await import('../../support/agent.ts');

const KEY = 'pairing-routes-key';
let forget: () => void;
let ip = 0;
/** A fresh client address, so one test's attempts do not rate limit another's. */
const nextIp = () => `10.0.0.${++ip}`;

/** A claim from one address. */
const claim = (code, address = nextIp()) =>
  callRoute(router, { method: 'POST', url: '/pairing/claim', body: { code }, extra: { ip: address } });

describe('pairing routes', () => {
  before(() => (forget = allowKey(KEY)));
  after(() => forget());
  beforeEach(() => pairing.reset());

  it('refuses to issue a code without an API key', async () => {
    const res = await callRoute(router, { method: 'POST', url: '/pairing' });
    assert.equal(res.status, 401);
  });

  it('issues a code bound to the caller’s key and default persona', async () => {
    const res = await callRoute(router, { method: 'POST', url: '/pairing', key: KEY, body: {} });
    assert.equal(res.status, 201);
    assert.ok(res.body.expiresAt > Date.now());
    const claimed = await claim(res.body.code);
    assert.equal(claimed.body.apiKey, KEY);
    assert.ok(claimed.body.persona);
  });

  it('answers 404 for a used, expired or unknown code', async () => {
    const res = await claim('A'.repeat(43));
    assert.equal(res.status, 404);
    assert.match(res.body.error, /invalid, used or expired/);
  });

  it('rate limits claims by client address', async () => {
    const address = nextIp();
    for (let i = 0; i < LIMITS.connect.burst; i++) await claim('A'.repeat(43), address);
    const res = await claim('A'.repeat(43), address);
    assert.equal(res.status, 429);
    assert.equal((await claim('A'.repeat(43))).status, 404, 'another address is unaffected');
  });
});
