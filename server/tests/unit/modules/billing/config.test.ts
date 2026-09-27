/**
 * Unit tests for billing's settings: hosted or not, plan prices, revocations.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { hosted, planOfPrice, pricesFor } from '../../../../src/modules/billing/config.ts';

/** The variables these tests set. */
const NAMES = ['STRIPE_SECRET_KEY', 'STRIPE_PRICES_DEVELOPER', 'STRIPE_PRICES_STARTUP'];

describe('billing config', () => {
  afterEach(() => NAMES.forEach((n) => delete process.env[n]));

  it('is hosted only where a Stripe key is set', () => {
    assert.equal(hosted(), false);
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    assert.equal(hosted(), true);
  });

  it('reads a plan’s prices as a trimmed comma list, base first', () => {
    process.env.STRIPE_PRICES_DEVELOPER = ' base_dev , min_dev,,mb_dev ';
    assert.deepEqual(pricesFor('developer'), ['base_dev', 'min_dev', 'mb_dev']);
  });

  it('finds the plan a base price belongs to, and Free for any other price', () => {
    process.env.STRIPE_PRICES_DEVELOPER = 'base_dev,min_dev';
    process.env.STRIPE_PRICES_STARTUP = 'base_start';
    assert.equal(planOfPrice('base_start'), 'startup');
    assert.equal(planOfPrice('base_dev'), 'developer');
    assert.equal(planOfPrice('min_dev'), 'free');
  });
});
