/**
 * Unit tests for desktop pairing codes: single use, short-lived, capped in
 * number, and only ever looked up when shaped like a code.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import * as pairing from '../../../../src/modules/pairing/service.ts';
import { TTL_MS, MAX_OUTSTANDING, MAX_CODE_LENGTH } from '../../../../src/modules/pairing/constants.ts';

describe('pairing codes', () => {
  beforeEach(() => {
    pairing.reset();
    mock.timers.enable({ apis: ['Date'], now: 1_700_000_000_000 });
  });
  afterEach(() => {
    mock.timers.reset();
    pairing.reset();
  });

  it('issues a code that redeems for the key and persona it was issued for', () => {
    const { code, expiresAt } = pairing.issue('key-a', 'p-7');
    assert.equal(expiresAt, Date.now() + TTL_MS);
    assert.deepEqual(pairing.claimDetails(code), { apiKey: 'key-a', persona: 'p-7' });
  });

  it('binds a code to the default persona when none is named', () => {
    const { code } = pairing.issue('key-a');
    assert.equal(pairing.claimDetails(code).persona, 'default');
  });

  it('redeems a code only once', () => {
    const { code } = pairing.issue('key-a');
    assert.equal(pairing.claim(code), 'key-a');
    assert.equal(pairing.claim(code), null);
  });

  it('refuses a code once its lifetime has passed', () => {
    const { code } = pairing.issue('key-a');
    mock.timers.tick(TTL_MS);
    assert.equal(pairing.claim(code), null);
  });

  it('refuses values that are not shaped like a code', () => {
    assert.equal(pairing.claim(undefined), null);
    assert.equal(pairing.claim(12345), null);
    assert.equal(pairing.claim('short'), null);
    assert.equal(pairing.claim('x'.repeat(MAX_CODE_LENGTH + 1)), null);
  });

  it('refuses a well-shaped code that was never issued', () => {
    pairing.issue('key-a');
    assert.equal(pairing.claim('A'.repeat(43)), null);
  });

  it('counts only codes still waiting to be claimed', () => {
    const { code } = pairing.issue('key-a');
    pairing.issue('key-b');
    assert.equal(pairing.outstanding(), 2);
    pairing.claim(code);
    assert.equal(pairing.outstanding(), 1);
    mock.timers.tick(TTL_MS);
    assert.equal(pairing.outstanding(), 0);
  });

  it('refuses a new code with 429 once the outstanding cap is reached', () => {
    for (let i = 0; i < MAX_OUTSTANDING; i++) pairing.issue('key-a');
    assert.throws(() => pairing.issue('key-a'), { status: 429 });
  });

  it('frees the cap as codes expire', () => {
    for (let i = 0; i < MAX_OUTSTANDING; i++) pairing.issue('key-a');
    mock.timers.tick(TTL_MS);
    assert.ok(pairing.issue('key-a').code);
  });
});
