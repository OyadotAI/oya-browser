/**
 * Unit tests for how the Routines tab words a routine (routines/format.ts): its
 * schedule, what it is doing, and its last run.
 */
import { describe, it, expect } from 'vitest';
import { lastRunLabel, nextLine, phaseOf, runSummary, scheduleOf } from '@/components/dashboard/routines/format';
import type { Routine } from '@/components/dashboard/routines/types';

const NOW = Date.parse('2026-01-01T12:00:00Z');
const HOUR = 3_600_000;

/** A routine with `over` laid on a cloud, hourly, never-run one. */
const routine = (over: Partial<Routine> = {}): Routine => ({
  id: 'r1',
  createdAt: NOW - HOUR / 2,
  name: 'Prices',
  prompt: 'Check prices',
  schedule: { kind: 'every', n: 1, unit: 'hours' },
  enabled: true,
  target: 'cloud',
  lastRunAt: null,
  nextRunAt: NOW + HOUR / 2,
  runs: [],
  ...over,
});

describe('routine format', () => {
  it('names every schedule kind it knows, and nothing for one it does not', () => {
    expect(scheduleOf(routine())).toBe('Every hour');
    expect(scheduleOf(routine({ schedule: { kind: 'every', n: 2, unit: 'hours' } }))).toBe('Every 2 hours');
    expect(scheduleOf(routine({ schedule: { kind: 'daily', at: '09:00' } }))).toBe('Daily at 09:00');
    expect(scheduleOf(routine({ schedule: { kind: 'weekly' as 'daily' } }))).toBe('');
  });

  it('reads running before off, and off before due', () => {
    const running = { id: 'x', startedAt: NOW, status: 'running' };
    expect(phaseOf(routine({ enabled: false, runs: [running] }), NOW)).toBe('running');
    expect(phaseOf(routine({ enabled: false, nextRunAt: null }), NOW)).toBe('off');
    expect(phaseOf(routine({ nextRunAt: NOW - 1 }), NOW)).toBe('due');
    expect(phaseOf(routine(), NOW)).toBe('scheduled');
  });

  it('stops counting a run as going once the server’s lease is over', () => {
    const stuck = { id: 'x', startedAt: NOW - 7 * HOUR, status: 'running' };
    expect(phaseOf(routine({ enabled: false, nextRunAt: null, runs: [stuck] }), NOW)).toBe('off');
    expect(phaseOf(routine({ runs: [stuck] }), NOW)).toBe('scheduled');
  });

  it('says where a running routine runs', () => {
    const runs = [{ id: 'x', startedAt: NOW, status: 'running' }];
    expect(nextLine(routine({ runs }), NOW)).toBe('Running in the cloud');
    expect(nextLine(routine({ runs, target: 'desktop' }), NOW)).toBe('Running on a desktop');
  });

  it('labels a routine never run, and a finished run with its time', () => {
    expect(lastRunLabel(routine(), NOW)).toBe('Never run');
    const done = { id: 'x', startedAt: NOW - 60_000, finishedAt: NOW - 18_000, status: 'done', steps: ['a', 'b'] };
    expect(lastRunLabel(routine({ runs: [done] }), NOW)).toMatch(/^Done · /);
    expect(runSummary(done, NOW)).toMatch(/ · 42s · 2 steps$/);
  });
});
