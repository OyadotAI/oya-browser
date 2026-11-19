/**
 * Unit tests for billing's settings: hosted or not, plan prices, revocations.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { hosted, planOfPrice, pricesFor } from '../../../../src/modules/billing/config.ts';

/** The variables these tests set. */
const NAMES = [
  'STRIPE_SECRET_KEY',
  'STRIPE_PRICES_DEVELOPER',
  'STRIPE_PRICES_STARTUP',
  'STRIPE_LEGACY_BASE_PRICES_DEVELOPER',
  'STRIPE_LEGACY_BASE_PRICES_STARTUP',
];

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

  it('recognizes earlier base prices without including them in checkout bundles', () => {
    process.env.STRIPE_PRICES_DEVELOPER = 'new_dev,min_dev';
    process.env.STRIPE_PRICES_STARTUP = 'new_start,min_start';
    process.env.STRIPE_LEGACY_BASE_PRICES_DEVELOPER = ' old_dev,older_dev,, ';
    process.env.STRIPE_LEGACY_BASE_PRICES_STARTUP = 'old_start';
    for (const price of ['new_dev', 'old_dev', 'older_dev']) assert.equal(planOfPrice(price), 'developer');
    for (const price of ['new_start', 'old_start']) assert.equal(planOfPrice(price), 'startup');
    for (const price of ['min_dev', 'min_start', 'unknown', '']) assert.equal(planOfPrice(price), 'free');
    assert.deepEqual(pricesFor('developer'), ['new_dev', 'min_dev']);
  });

  it('retains existing plan recognition when new subscriptions are not on sale', () => {
    process.env.STRIPE_LEGACY_BASE_PRICES_DEVELOPER = 'old_dev';
    assert.equal(planOfPrice('old_dev'), 'developer');
    assert.deepEqual(pricesFor('developer'), []);
  });
});
