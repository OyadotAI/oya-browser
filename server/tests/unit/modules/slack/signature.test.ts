/**
 * Unit tests for Slack request signing: the v0 HMAC over the raw body, the
 * five-minute replay window, and the constant-time comparison under it.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import { verifySignature, sameText } from '../../../../src/modules/slack/signature.ts';
import { SIGNATURE_WINDOW_SECONDS } from '../../../../src/modules/slack/constants.ts';

const SECRET = 'signing-secret';
const NOW_S = 1_700_000_000;
const BODY = 'payload=%7B%22actions%22%3A%5B%5D%7D';

/** Slack's signature header for a body at a timestamp. */
const sign = (body, timestamp, secret = SECRET) =>
  `v0=${createHmac('sha256', secret).update(`v0:${timestamp}:${body}`).digest('hex')}`;
/** The two headers Slack sends. */
const headers = (timestamp, signature) => ({
  'x-slack-request-timestamp': String(timestamp),
  'x-slack-signature': signature,
});

describe('verifySignature', () => {
  beforeEach(() => mock.timers.enable({ apis: ['Date'], now: NOW_S * 1000 }));
  afterEach(() => mock.timers.reset());

  it('accepts a body signed with the signing secret inside the window', () => {
    assert.equal(verifySignature(BODY, headers(NOW_S, sign(BODY, NOW_S)), SECRET), true);
  });

  it('accepts a timestamp at the edge of the replay window', () => {
    const ts = NOW_S - SIGNATURE_WINDOW_SECONDS;
    assert.equal(verifySignature(BODY, headers(ts, sign(BODY, ts)), SECRET), true);
  });

  it('refuses a stale timestamp even when the signature over it is correct', () => {
    const ts = NOW_S - SIGNATURE_WINDOW_SECONDS - 1;
    assert.equal(verifySignature(BODY, headers(ts, sign(BODY, ts)), SECRET), false);
  });

  it('refuses a timestamp from the future beyond the window', () => {
    const ts = NOW_S + SIGNATURE_WINDOW_SECONDS + 1;
    assert.equal(verifySignature(BODY, headers(ts, sign(BODY, ts)), SECRET), false);
  });

  it('refuses a tampered body', () => {
    const signature = sign(BODY, NOW_S);
    assert.equal(verifySignature(`${BODY}&x=1`, headers(NOW_S, signature), SECRET), false);
  });

  it('refuses a signature made with another secret', () => {
    assert.equal(verifySignature(BODY, headers(NOW_S, sign(BODY, NOW_S, 'other')), SECRET), false);
  });

  it('refuses a request missing either header', () => {
    assert.equal(verifySignature(BODY, { 'x-slack-signature': sign(BODY, NOW_S) }, SECRET), false);
    assert.equal(verifySignature(BODY, { 'x-slack-request-timestamp': String(NOW_S) }, SECRET), false);
  });

  it('refuses everything when no signing secret is configured', () => {
    const saved = process.env.SLACK_SIGNING_SECRET;
    delete process.env.SLACK_SIGNING_SECRET;
    try {
      assert.equal(verifySignature(BODY, headers(NOW_S, sign(BODY, NOW_S))), false);
    } finally {
      if (saved !== undefined) process.env.SLACK_SIGNING_SECRET = saved;
    }
  });

  it('reads the signing secret from SLACK_SIGNING_SECRET by default', () => {
    const saved = process.env.SLACK_SIGNING_SECRET;
    process.env.SLACK_SIGNING_SECRET = SECRET;
    try {
      assert.equal(verifySignature(BODY, headers(NOW_S, sign(BODY, NOW_S))), true);
    } finally {
      if (saved === undefined) delete process.env.SLACK_SIGNING_SECRET;
      else process.env.SLACK_SIGNING_SECRET = saved;
    }
  });
});

describe('sameText', () => {
  it('is true for equal strings', () => {
    assert.equal(sameText('abc', 'abc'), true);
  });

  it('is false for strings of different length rather than throwing', () => {
    assert.equal(sameText('abc', 'abcd'), false);
  });

  it('is false for equal-length strings that differ', () => {
    assert.equal(sameText('abc', 'abd'), false);
  });
});
