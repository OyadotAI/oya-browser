/**
 * Unit tests for rate limits and quotas: token buckets that refill over time,
 * the 429 answer with its headers, point-in-time and hourly quotas, and
 * limits switched off by configuration.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  LIMITS,
  QUOTAS,
  checkHourly,
  checkQuota,
  consume,
  enforce,
  reset,
  status,
} from '../../../src/platform/limits.ts';
import * as usage from '../../../src/platform/usage.ts';
import { MS_PER_MINUTE } from '../../../src/platform/constants.ts';
import { FakeResponse, fakeRequest } from '../support/browsers.ts';

/** Spends every token `key` has on the command limit. */
function exhaust(key: string) {
  for (let i = 0; i < LIMITS.command.burst; i++) consume('command', key);
}

describe('consume', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['Date'], now: 1_000_000 });
    reset();
    usage.reset();
  });
  afterEach(() => mock.timers.reset());

  it('allows up to the burst and reports what remains', () => {
    const first = consume('command', 'k1');
    assert.equal(first.allowed, true);
    assert.equal(first.limit, LIMITS.command.perMinute);
    assert.equal(first.remaining, LIMITS.command.burst - 1);
  });

  it('refuses once the burst is spent, saying when to retry', () => {
    exhaust('k2');
    const refused = consume('command', 'k2');
    assert.equal(refused.allowed, false);
    assert.equal(refused.remaining, 0);
    assert.equal(refused.retryAfter, Math.max(1, Math.ceil(60 / LIMITS.command.perMinute)));
  });

  it('counts a refusal against the key’s usage', () => {
    exhaust('k3');
    consume('command', 'k3');
    assert.equal(usage.current('k3').rate_limited, 1);
  });

  it('refills tokens as time passes', () => {
    exhaust('k4');
    mock.timers.tick(MS_PER_MINUTE);
    assert.equal(consume('command', 'k4').allowed, true);
  });

  it('keeps each key’s bucket separate', () => {
    exhaust('k5');
    assert.equal(consume('command', 'k6').allowed, true);
  });

  it('always allows a call with no key or an unknown limit', () => {
    assert.equal(consume('command', '').allowed, true);
    assert.equal(consume('no-such-limit', 'k').remaining, Infinity);
  });

  it('allows everything while a limit is disabled', () => {
    const saved = LIMITS.chat.perMinute;
    LIMITS.chat.perMinute = 0;
    try {
      assert.deepEqual(consume('chat', 'k7'), { allowed: true, limit: 0, remaining: Infinity, retryAfter: 0 });
      assert.equal(status('k7').limits.chat.disabled, true);
    } finally {
      LIMITS.chat.perMinute = saved;
    }
  });
});

describe('quotas', () => {
  beforeEach(() => usage.reset());

  it('allows while the current count is under the quota', () => {
    assert.deepEqual(checkQuota('browsers', 'k', 0), { allowed: true, quota: QUOTAS.browsers, current: 0 });
  });

  it('refuses at the quota and records the denial', () => {
    const out = checkQuota('browsers', 'kq', QUOTAS.browsers);
    assert.equal(out.allowed, false);
    assert.equal(usage.current('kq').quota_denied, 1);
  });

  it('allows anything for a quota that is not configured', () => {
    assert.deepEqual(checkQuota('nothing', 'k', 99), { allowed: true, quota: 0, current: 99 });
  });

  it('sums input and output chat tokens for the hourly token quota', () => {
    usage.record('kh', 'chat_input_tokens', 5);
    usage.record('kh', 'chat_output_tokens', 7);
    assert.equal(checkHourly('chatTokensPerHour', 'kh').current, 12);
  });

  it('counts sandboxes created this hour for the sandbox quota', () => {
    usage.record('ks', 'sandboxes_created', 2);
    assert.equal(checkHourly('sandboxesPerHour', 'ks').current, 2);
  });

  it('allows an hourly check it does not know', () => {
    assert.deepEqual(checkHourly('mystery', 'k'), { allowed: true, quota: 0, current: 0 });
  });
});

describe('enforce', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['Date'], now: 1_000_000 });
    reset();
  });
  afterEach(() => mock.timers.reset());

  it('passes the request on with RateLimit headers', () => {
    const res = new FakeResponse();
    const next = mock.fn();
    enforce('command')(fakeRequest({ key: 'ke' }), res, next);
    assert.equal(next.mock.callCount(), 1);
    assert.equal(res.headers['RateLimit-Limit'], String(LIMITS.command.perMinute));
    assert.equal(res.headers['RateLimit-Remaining'], String(LIMITS.command.burst - 1));
  });

  it('answers 429 with Retry-After once the key is over its limit', () => {
    exhaust('kf');
    const res = new FakeResponse();
    const next = mock.fn();
    enforce('command')(fakeRequest({ key: 'kf' }), res, next);
    assert.equal(next.mock.callCount(), 0);
    assert.equal(res.statusCode, 429);
    assert.equal(res.body.error, 'Rate limit exceeded for command');
    assert.equal(res.headers['Retry-After'], String(res.body.retryAfter));
  });

  it('reports 0 remaining rather than Infinity for an unlimited caller', () => {
    const res = new FakeResponse();
    enforce('command')(fakeRequest({ key: '' }), res, () => {});
    assert.equal(res.headers['RateLimit-Remaining'], '0');
  });
});

describe('status', () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ['Date'], now: 1_000_000 });
    reset();
  });
  afterEach(() => mock.timers.reset());

  it('reports a full bucket for a key that has spent nothing, without consuming', () => {
    const out = status('fresh');
    assert.equal(out.limits.command.remaining, LIMITS.command.burst);
    assert.equal(status('fresh').limits.command.remaining, LIMITS.command.burst);
    assert.equal(out.quotas, QUOTAS);
  });

  it('reports what a key has left after spending', () => {
    consume('command', 'spender');
    assert.equal(status('spender').limits.command.remaining, LIMITS.command.burst - 1);
  });
});
