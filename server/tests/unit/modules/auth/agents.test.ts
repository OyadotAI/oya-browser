/**
 * Unit tests for agent self-signup's guards: a challenge is solved once,
 * before it expires, by a nonce that meets the difficulty; a forged or
 * expired challenge is refused; the email and caller address are read as
 * the rules say.
 */
import { afterEach, describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Status } from '../../../../src/platform/http-status.ts';
import {
  claimUrl,
  clientAddress,
  issueChallenge,
  solves,
  spendChallenge,
  validEmail,
} from '../../../../src/modules/auth/agents.ts';

/** The first nonce that solves `challenge`, found the way an agent would. */
function solve(challenge: string) {
  for (let n = 0; ; n++) if (solves(challenge, String(n))) return String(n);
}

/** Asserts `fn` throws a 400 whose message matches `message`. */
const refused = (fn: () => void, message: RegExp) =>
  assert.throws(fn, (err: any) => err.status === Status.BAD_REQUEST && message.test(err.message));

afterEach(() => mock.timers.reset());

describe('issueChallenge', () => {
  it('names the difficulty and when the challenge expires', () => {
    const issued = issueChallenge();
    assert.equal(issued.difficulty, 5);
    assert.ok(Date.parse(issued.expires_at) > Date.now());
  });
});

describe('spendChallenge', () => {
  it('accepts a solved challenge once, and refuses it the second time', () => {
    const { challenge } = issueChallenge();
    const nonce = solve(challenge);
    spendChallenge(challenge, nonce);
    refused(() => spendChallenge(challenge, nonce), /already used/);
  });

  it('refuses a nonce that does not solve it', () => {
    const { challenge } = issueChallenge();
    const wrong = ['a', 'b', 'c', 'd'].find((n) => !solves(challenge, n));
    refused(() => spendChallenge(challenge, wrong), /does not solve/);
    refused(() => spendChallenge(challenge, 7), /does not solve/);
    refused(() => spendChallenge(challenge, 'x'.repeat(65)), /does not solve/);
  });

  it('refuses a challenge this server did not seal', () => {
    refused(() => spendChallenge('made-up', '0'), /invalid or expired/);
    refused(() => spendChallenge(undefined, '0'), /invalid or expired/);
  });

  it('refuses a challenge solved after it expired', () => {
    mock.timers.enable({ apis: ['Date'], now: Date.now() });
    const { challenge } = issueChallenge();
    const nonce = solve(challenge);
    mock.timers.tick(600_001);
    refused(() => spendChallenge(challenge, nonce), /invalid or expired/);
  });
});

describe('validEmail', () => {
  it('takes an address and refuses anything else', () => {
    assert.equal(validEmail('owner@example.com'), true);
    for (const bad of ['', 'owner', 'a@b', 'a b@c.d', 42, `${'a'.repeat(250)}@x.io`])
      assert.equal(validEmail(bad), false);
  });
});

describe('clientAddress', () => {
  it('trusts the hop the ingress appended, not what the client claimed before it', () => {
    const req = { headers: { 'x-forwarded-for': '6.6.6.6, 203.0.113.9' }, socket: { remoteAddress: '10.0.0.1' } };
    assert.equal(clientAddress(req), '203.0.113.9');
  });

  it('falls back to the socket without a forwarded header', () => {
    assert.equal(clientAddress({ headers: {}, socket: { remoteAddress: '10.0.0.1' } }), '10.0.0.1');
    assert.equal(clientAddress({ headers: {} }), 'unknown');
  });
});

describe('claimUrl', () => {
  it('carries the key in the fragment, which never reaches a server', () => {
    assert.match(claimUrl('k'.repeat(32)), /^https?:\/\/[^#]+\/claim#k{32}$/);
  });
});
