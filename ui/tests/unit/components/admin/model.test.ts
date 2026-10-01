/**
 * Unit tests for the admin page's display numbers.
 */
import { describe, it, expect } from 'vitest';
import {
  barHeights,
  changeText,
  dayOf,
  daysFromNow,
  direction,
  dollars,
  downloadsByDay,
  hours,
} from '@/components/admin/model';

describe('admin model', () => {
  it('adds each day’s downloads across platforms, newest day first, ignoring kinds it does not know', () => {
    const rows = [
      { day: '2026-03-14', kind: 'installer', platform: 'mac', count: 2 },
      { day: '2026-03-14', kind: 'installer', platform: 'windows', count: 3 },
      { day: '2026-03-15', kind: 'update_check', platform: 'mac', count: 9 },
      { day: '2026-03-15', kind: 'other', platform: 'mac', count: 1 },
    ];
    expect(downloadsByDay(rows)).toEqual([
      { day: '2026-03-15', installer: 0, update: 0, update_check: 9 },
      { day: '2026-03-14', installer: 5, update: 0, update_check: 0 },
    ]);
  });

  it('shows hours to one decimal, a day from an ISO time, and a date some days on', () => {
    expect(hours(5400)).toBe('1.5');
    expect(hours()).toBe('0.0');
    expect(dayOf('2026-03-14T10:00:00Z')).toBe('2026-03-14');
    expect(dayOf(null)).toBe('—');
    expect(daysFromNow(1, Date.UTC(2026, 0, 31))).toBe('2026-02-01');
  });

  it('reads a change as a signed percent, "new" from nothing, and "flat" otherwise', () => {
    expect(changeText({ now: 12, before: 10, change: 20 })).toBe('+20%');
    expect(changeText({ now: 8, before: 10, change: -20 })).toBe('-20%');
    expect(changeText({ now: 3, before: 0, change: null })).toBe('new');
    expect(changeText({ now: 0, before: 0, change: null })).toBe('flat');
    expect(changeText({ now: 5, before: 5, change: 0 })).toBe('flat');
  });

  it('points a change up, down or flat, counting something from nothing as up', () => {
    expect(direction({ now: 3, before: 0, change: null })).toBe('up');
    expect(direction({ now: 1, before: 2, change: -50 })).toBe('down');
    expect(direction({ now: 0, before: 0, change: null })).toBe('flat');
  });

  it('shows cents as whole dollars', () => {
    expect(dollars(1234567)).toBe('$12,346');
  });

  it('sizes bars against the tallest, giving any non-zero day a sliver and an empty one none', () => {
    expect(barHeights([0, 1, 50, 100])).toEqual([0, 4, 50, 100]);
    expect(barHeights([0, 0])).toEqual([0, 0]);
  });
});
