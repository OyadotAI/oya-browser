/**
 * Unit tests for the auth constants read from the environment: the operator's
 * cap on a new API key's lifetime, OYA_API_KEY_MAX_DAYS.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { apiKeyMaxDays } from '../../../../src/modules/auth/constants.ts';

describe('apiKeyMaxDays', () => {
  it('sets no cap when OYA_API_KEY_MAX_DAYS is unset or blank', () => {
    assert.equal(apiKeyMaxDays(undefined), null);
    assert.equal(apiKeyMaxDays(' '), null);
  });

  it('reads a whole number of days in range', () => {
    assert.equal(apiKeyMaxDays('90'), 90);
  });

  for (const bad of ['0', '3651', '1.5', 'ninety'])
    it(`stops the server on ${JSON.stringify(bad)} rather than silently lifting the cap`, () => {
      assert.throws(() => apiKeyMaxDays(bad), /OYA_API_KEY_MAX_DAYS must be a whole number of days from 1 to 3650/);
    });
});
