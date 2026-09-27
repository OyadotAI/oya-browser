/**
 * Unit tests for where a person stands: which plan applies and since when.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { monthStart, standingOf } from '../../../../src/modules/billing/standing.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);

describe('standingOf', () => {
  it('puts a person with no subscription on Free for the calendar month', () => {
    assert.deepEqual(standingOf('u1', null, NOW), {
      userId: 'u1',
      plan: 'free',
      status: null,
      since: '2026-03-01T00:00:00.000Z',
    });
  });

  it('keeps an active subscription’s plan and period', () => {
    const row = { user_id: 'u1', plan: 'developer', status: 'active', period_start: '2026-03-10T00:00:00.000Z' };
    assert.deepEqual(standingOf('u1', row, NOW), {
      userId: 'u1',
      plan: 'developer',
      status: 'active',
      since: row.period_start,
    });
  });

  it('keeps a late payer’s plan, so admission can say why it refuses', () => {
    assert.equal(standingOf('u1', { user_id: 'u1', plan: 'startup', status: 'past_due' }, NOW).status, 'past_due');
  });

  it('puts a canceled subscription back on Free', () => {
    assert.equal(standingOf('u1', { user_id: 'u1', plan: 'developer', status: 'canceled' }, NOW).plan, 'free');
  });

  it('treats a plan name it does not know as Free', () => {
    assert.equal(standingOf('u1', { user_id: 'u1', plan: 'platinum', status: 'active' }, NOW).plan, 'free');
  });

  it('starts a period at the month when a subscription has no period yet', () => {
    assert.equal(standingOf('u1', { user_id: 'u1', plan: 'developer', status: 'active' }, NOW).since, monthStart(NOW));
  });
});
