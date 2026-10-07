/** Complimentary access is capped and support grants extend real admission allowances. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { includedFor } from '../../../../src/modules/billing/adjustments.ts';
import { standingOf } from '../../../../src/modules/billing/standing.ts';
import { PLANS } from '../../../../src/modules/billing/constants.ts';

it('adds grants to a free plan while preserving the other limits', () => {
  const included = includedFor(
    { userId: 'u', plan: 'free', status: null, since: 'period' },
    { cloud_seconds: 3600, hosted_llm_microusd: 1_000_000 },
  );
  assert.equal(included.cloudSeconds, PLANS.free.cloudSeconds + 3600);
  assert.equal(included.llmMicroUsd, PLANS.free.llmMicroUsd + 1_000_000);
  assert.equal(included.concurrent, PLANS.free.concurrent);
});

it('caps complimentary paid plans rather than granting unlimited unbilled use', () => {
  const included = includedFor({ userId: 'u', plan: 'developer', status: 'admin', since: 'period' });
  assert.equal(included.overage, false);
  assert.equal(included.cloudSeconds, PLANS.developer.cloudSeconds);
});

it('uses the current month for an override after a subscription was canceled', () => {
  const standing = standingOf(
    'u',
    { user_id: 'u', plan: 'free', status: 'canceled', period_start: '2020-01-01', admin_plan: 'developer' },
    Date.UTC(2026, 9, 7),
  );
  assert.equal(standing.since, '2026-10-01T00:00:00.000Z');
});
