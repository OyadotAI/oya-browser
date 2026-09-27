/**
 * Unit tests for the admin overview's numbers, worked out from rows.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import * as view from '../../../../src/modules/admin/overview.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);

describe('admin overview', () => {
  it('counts rows per day from a day on, oldest first', () => {
    const rows = [
      { at: '2026-03-14T01:00:00Z' },
      { at: '2026-03-14T09:00:00Z' },
      { at: '2026-03-10T00:00:00Z' },
      { at: '2026-01-01T00:00:00Z' },
    ];
    assert.deepEqual(view.perDay(rows, 'at', view.daysAgo(NOW, 30)), [
      { day: '2026-03-10', count: 1 },
      { day: '2026-03-14', count: 2 },
    ]);
  });

  it('counts paying people by plan, and those late', () => {
    const subs = [
      { plan: 'developer', status: 'active' },
      { plan: 'developer', status: 'past_due' },
      { plan: 'startup', status: 'trialing' },
      { plan: 'startup', status: 'canceled' },
      { plan: 'free', status: 'canceled' },
    ];
    assert.deepEqual(view.plans(subs), { byPlan: { developer: 2, startup: 1 }, pastDue: 1 });
  });

  it('sums each person’s own rows, ignoring key rows, and lists the heaviest with their email', () => {
    const rows = [
      { api_key: 'u:a', cloud_seconds: 60, agent_steps: 1 },
      { api_key: 'u:a', cloud_seconds: 60, agent_steps: 1 },
      { api_key: 'u:b', cloud_seconds: 600, agent_steps: 0 },
      { api_key: 'fingerprint', cloud_seconds: 9999 },
    ];
    const people = view.perPerson(rows);
    const top = view.top(people, 'cloud_seconds', new Map([['a', 'a@x.com']]));
    assert.deepEqual(
      top.map((p) => [p.userId, p.cloud_seconds, p.email]),
      [
        ['b', 600, null],
        ['a', 120, 'a@x.com'],
      ],
    );
    assert.deepEqual(
      view.top(people, 'agent_steps', new Map()).map((p) => p.userId),
      ['a'],
    );
  });

  it('counts installs, those seen this week, and unlicensed ones past the free cap', () => {
    const installs = [
      { last_seen: '2026-03-14T00:00:00Z', peak_cloud: 9, license_id: null },
      { last_seen: '2026-03-14T00:00:00Z', peak_cloud: 9, license_id: 'L' },
      { last_seen: '2026-03-14T00:00:00Z', peak_cloud: 2, license_id: null },
      { last_seen: '2026-01-01T00:00:00Z', peak_cloud: 50, license_id: null },
    ];
    assert.deepEqual(view.installSummary(installs, NOW, 5), { total: 4, active: 3, overCap: 1 });
  });

  it('counts the fleet in all, in the cloud, and by provider', () => {
    const browsers = [{ provider: 'oya-cloud' }, { provider: 'oya-desktop' }, { provider: 'oya-desktop' }, {}];
    assert.deepEqual(view.fleet(browsers, new Set(['oya-cloud'])), {
      total: 4,
      cloud: 1,
      byProvider: { 'oya-cloud': 1, 'oya-desktop': 2, unknown: 1 },
    });
  });
});
