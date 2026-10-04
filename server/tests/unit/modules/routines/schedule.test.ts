/**
 * Unit tests for when the server runs a cloud routine next: "every N" counts
 * from the last run, and "daily" is read in the routine's own time zone,
 * across a daylight-saving change.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { isDue, nextDaily, nextRunAt } from '../../../../src/modules/routines/schedule.ts';
import { ROUTINE_RUN_LEASE_MS } from '../../../../src/modules/routines/constants.ts';

const HOUR = 3_600_000;
const base = {
  id: 'r',
  createdAt: Date.UTC(2026, 0, 1),
  name: 'n',
  prompt: 'p',
  enabled: true,
  target: 'cloud' as const,
  lastRunAt: null,
  runs: [],
};

describe('cloud routine schedule', () => {
  it('runs "every N" N units after the last run, or after it was made', () => {
    const every = { ...base, schedule: { kind: 'every' as const, n: 2, unit: 'hours' } };
    assert.equal(nextRunAt(every), base.createdAt + 2 * HOUR);
    assert.equal(nextRunAt({ ...every, lastRunAt: base.createdAt + HOUR }), base.createdAt + 3 * HOUR);
  });

  it('reads a daily time in the routine’s time zone', () => {
    const since = Date.UTC(2026, 0, 10, 12, 0); // 07:00 in New York
    assert.equal(nextDaily('09:30', 'America/New_York', since), Date.UTC(2026, 0, 10, 14, 30));
    assert.equal(nextDaily('09:30', 'UTC', since), Date.UTC(2026, 0, 11, 9, 30), 'already past today: tomorrow');
    assert.equal(nextDaily('08:00', 'Asia/Tokyo', since), Date.UTC(2026, 0, 10, 23, 0));
  });

  it('keeps the local time across a daylight-saving change', () => {
    const beforeSpring = Date.UTC(2026, 2, 7, 20, 0); // Sat 15:00 EST; clocks go forward Sunday
    assert.equal(nextDaily('09:00', 'America/New_York', beforeSpring), Date.UTC(2026, 2, 8, 13, 0), '09:00 EDT');
    const beforeFall = Date.UTC(2026, 9, 24, 12, 0); // London, clocks go back Sunday the 25th
    assert.equal(nextDaily('09:00', 'Europe/London', beforeFall + 24 * HOUR), Date.UTC(2026, 9, 26, 9, 0));
  });

  it('is due once its time has come, unless it is off or a run is still going', () => {
    const r = { ...base, schedule: { kind: 'every' as const, n: 1, unit: 'hours' } };
    const now = base.createdAt + HOUR;
    assert.equal(isDue(r, now - 1), false);
    assert.equal(isDue(r, now), true);
    assert.equal(isDue({ ...r, enabled: false }, now), false);
    const going = { id: 'x', startedAt: now - 1, status: 'running' };
    assert.equal(isDue({ ...r, runs: [going] }, now), false);
    assert.equal(
      isDue({ ...r, runs: [{ ...going, startedAt: now - ROUTINE_RUN_LEASE_MS }] }, now),
      true,
      'lease ran out',
    );
  });
});
