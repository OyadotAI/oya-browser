/**
 * Unit tests for reporting paying people's usage to Stripe's meters.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { UsageReporter } from '../../../../src/modules/billing/reporter.ts';

/** A reporter over rows in memory, usage by period, and a Stripe that records its calls, failing call number `failOn`. */
function fixture(rows: any[], usedByPeriod: Record<string, Record<string, number>>, failOn = -1) {
  const events: any[] = [];
  const saved: any[] = [];
  let calls = 0;
  const stripe = {
    post: async (_path, params) => {
      if (calls++ === failOn) throw new Error('stripe down');
      events.push(params);
      return {};
    },
  };
  const deps = {
    all: async () => rows,
    saveReported: async (userId, reported) => void saved.push({ userId, reported }),
    usageSince: async (_id, since) => usedByPeriod[since] || {},
    stripe,
  };
  return { r: new UsageReporter(deps), events, saved };
}

/** A paying person's row. */
const row = (extra = {}) => ({
  user_id: 'u1',
  status: 'active',
  stripe_customer_id: 'cus_1',
  period_start: 'P1',
  ...extra,
});

/** Each event as [meter, value]. */
const sent = (events: any[]) => events.map((e) => [e.event_name, e.payload.value]);

describe('UsageReporter', () => {
  it('sends each meter’s total in its unit: minutes, MB, cents of model cost and steps', async () => {
    const used = {
      cloud_seconds: 150,
      residential_proxy_bytes: 3 * 1_048_576,
      hosted_llm_microusd: 70_000,
      agent_steps: 4,
    };
    const { r, events, saved } = fixture([row()], { P1: used });
    await r.report();
    assert.deepEqual(sent(events), [
      ['oya_cloud_minutes', 2],
      ['oya_proxy_mb', 3],
      ['oya_model_cents', 7],
      ['oya_agent_steps', 4],
    ]);
    assert.equal(events[0].payload.stripe_customer_id, 'cus_1');
    assert.deepEqual(saved.at(-1).reported, {
      period: 'P1',
      oya_cloud_minutes: 2,
      oya_proxy_mb: 3,
      oya_model_cents: 7,
      oya_agent_steps: 4,
    });
  });

  it('sends only the growth since the last report, and nothing when there is none', async () => {
    const reported = { period: 'P1', oya_cloud_minutes: 2, oya_agent_steps: 4 };
    const { r, events } = fixture([row({ reported })], { P1: { cloud_seconds: 300, agent_steps: 4 } });
    await r.report();
    assert.deepEqual(sent(events), [['oya_cloud_minutes', 3]]);
  });

  it('records each meter as soon as it is sent, so a failure part way never sends the same growth twice', async () => {
    const { r, saved } = fixture([row()], { P1: { cloud_seconds: 120, agent_steps: 5 } }, 1);
    const original = console.error;
    console.error = () => {};
    await r.report().finally(() => (console.error = original));
    assert.deepEqual(saved.at(-1).reported, { period: 'P1', oya_cloud_minutes: 2 });
  });

  it('settles what an ended period grew before starting the new one from nothing', async () => {
    const reported = { period: 'P0', oya_agent_steps: 100 };
    const { r, events, saved } = fixture([row({ reported })], { P0: { agent_steps: 130 }, P1: { agent_steps: 5 } });
    await r.report();
    assert.deepEqual(sent(events), [
      ['oya_agent_steps', 30],
      ['oya_agent_steps', 5],
    ]);
    assert.deepEqual(saved.at(-1).reported, { period: 'P1', oya_agent_steps: 5 });
  });

  it('names each event after the total it reaches, so a retried report is one Stripe already has', async () => {
    const first = fixture([row()], { P1: { agent_steps: 5 } });
    const again = fixture([row()], { P1: { agent_steps: 5 } });
    await first.r.report();
    await again.r.report();
    assert.equal(first.events[0].identifier, again.events[0].identifier);
    assert.ok(first.events[0].identifier.length <= 100);
  });

  it('reports only subscriptions that are billed and known to Stripe', async () => {
    const rows = [row({ status: 'canceled' }), row({ stripe_customer_id: null }), row({ period_start: null })];
    const { r, events } = fixture(rows, { P1: { agent_steps: 5 } });
    await r.report();
    assert.equal(events.length, 0);
  });

  it('logs a failed report and goes on to the next person', async () => {
    const { r, events } = fixture([row(), row({ user_id: 'u2' })], { P1: { agent_steps: 5 } }, 0);
    const logged: string[] = [];
    const original = console.error;
    console.error = (m) => void logged.push(m);
    await r.report().finally(() => (console.error = original));
    assert.match(logged[0], /stripe down/);
    assert.equal(events.length, 1);
  });
});

it('subtracts period grants from cloud minutes and hosted model cost before reporting', async () => {
  const { r, events } = fixture([row()], { P1: { cloud_seconds: 7200, hosted_llm_microusd: 2_000_000 } });
  r.deps.creditsFor = async (_id, since) => {
    assert.equal(since, 'P1');
    return { cloud_seconds: 3600, hosted_llm_microusd: 500_000 };
  };
  await r.report();
  assert.deepEqual(sent(events), [
    ['oya_cloud_minutes', 60],
    ['oya_model_cents', 150],
  ]);
});

it('does not send negative usage or retract usage already reported when grants exceed it', async () => {
  const { r, events } = fixture([row({ reported: { period: 'P1', oya_cloud_minutes: 60 } })], {
    P1: { cloud_seconds: 7200 },
  });
  r.deps.creditsFor = async () => ({ cloud_seconds: 10_000 });
  await r.report();
  assert.deepEqual(events, []);
});

it('uses grants from the correct period when settling a rollover', async () => {
  const { r, events } = fixture([row({ reported: { period: 'P0' } })], {
    P0: { cloud_seconds: 7200 },
    P1: { cloud_seconds: 7200 },
  });
  r.deps.creditsFor = async (_id, since) => ({ cloud_seconds: since === 'P0' ? 3600 : 0 });
  await r.report();
  assert.deepEqual(sent(events), [
    ['oya_cloud_minutes', 60],
    ['oya_cloud_minutes', 120],
  ]);
});
