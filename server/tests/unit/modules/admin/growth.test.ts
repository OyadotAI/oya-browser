/**
 * Unit tests for the admin page's growth numbers: the day by day series,
 * week over week and day over day, and the people reached.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import * as growth from '../../../../src/modules/admin/growth.ts';

/** 15 March 2026, midday UTC. */
const NOW = Date.UTC(2026, 2, 15, 12);

/** A key's stored hash, and the fingerprint its usage rows carry. */
const HASH = createHash('sha256').update('key-1').digest('hex');
const PRINT = HASH.slice(0, 16);

/** Sources with nothing in them, to fill in per test. */
const empty = (): growth.Sources => ({ profiles: [], usage: [], keys: [], downloads: [], installs: [], payments: [] });

describe('admin growth', () => {
  it('has one row per day shown, oldest first, zero on quiet days', () => {
    const days = growth.daily(empty(), NOW, 3);
    assert.deepEqual(
      days.map((d) => d.day),
      ['2026-03-13', '2026-03-14', '2026-03-15'],
    );
    assert.equal(days[0].signups, 0);
  });

  it('counts a person active once a day, whether through a key or their own row', () => {
    const s = empty();
    s.keys = [{ key_hash: HASH, user_id: 'u1' }];
    s.usage = [
      { api_key: PRINT, hour: '2026-03-15T09:00:00.000Z', commands: 4 },
      { api_key: 'u:u1', hour: '2026-03-15T10:00:00.000Z', agent_steps: 2 },
      { api_key: 'u:u2', hour: '2026-03-14T10:00:00.000Z', cloud_seconds: 60 },
      { api_key: 'unknown', hour: '2026-03-15T10:00:00.000Z', commands: 9 },
      { api_key: 'u:u3', hour: '2026-03-15T10:00:00.000Z', commands: 0 },
    ];
    const days = growth.daily(s, NOW, 2);
    assert.deepEqual(
      days.map((d) => d.active),
      [1, 1],
    );
    assert.deepEqual(growth.reach(growth.activePeople(s.usage, s.keys), NOW), { today: 1, week: 2, month: 2 });
  });

  it('takes steps and cloud time from people’s rows and commands from key rows, so nothing counts twice', () => {
    const s = empty();
    s.usage = [
      { api_key: PRINT, hour: '2026-03-15T09:00:00.000Z', agent_steps: 5, commands: 3 },
      { api_key: 'u:u1', hour: '2026-03-15T09:00:00.000Z', agent_steps: 5, cloud_seconds: 120 },
    ];
    const [today] = growth.daily(s, NOW, 1);
    assert.equal(today.agent_steps, 5);
    assert.equal(today.cloud_seconds, 120);
    assert.equal(today.commands, 3);
  });

  it('counts signups, installer downloads, new installs and revenue on their days', () => {
    const s = empty();
    s.profiles = [{ created_at: '2026-03-15T01:00:00Z' }, { created_at: '2026-03-15T02:00:00Z' }];
    s.downloads = [
      { day: '2026-03-15', kind: 'installer', count: 7 },
      { day: '2026-03-15', kind: 'update', count: 9 },
    ];
    s.installs = [{ first_seen: '2026-03-15T03:00:00Z' }];
    s.payments = [{ day: '2026-03-15', cents: 2000 }];
    const [today] = growth.daily(s, NOW, 1);
    assert.deepEqual(
      [today.signups, today.installers, today.update_checks, today.new_installs, today.revenue_cents],
      [2, 7, 0, 1, 2000],
    );
  });

  it('compares a stretch with the one before, in percent, and has no percent from zero', () => {
    const days = [1, 1, 2, 4].map((signups, i) => ({ day: String(i), signups }) as growth.Day);
    assert.deepEqual(growth.compare(days, 'signups', 2), { now: 6, before: 2, change: 200 });
    const quiet = [0, 3].map((signups, i) => ({ day: String(i), signups }) as growth.Day);
    assert.equal(growth.compare(quiet, 'signups', 1).change, null);
  });

  it('compares day over day on whole days, leaving today out', () => {
    const s = empty();
    s.profiles = [
      { created_at: '2026-03-13T01:00:00Z' },
      { created_at: '2026-03-14T01:00:00Z' },
      { created_at: '2026-03-14T02:00:00Z' },
      { created_at: '2026-03-15T01:00:00Z' },
    ];
    const t = growth.trends(growth.daily(s, NOW, 14));
    assert.deepEqual(t.day.signups, { now: 2, before: 1, change: 100 });
    assert.equal(t.week.signups.now, 4);
  });
});
