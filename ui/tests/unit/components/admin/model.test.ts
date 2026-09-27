/**
 * Unit tests for the admin page's display numbers.
 */
import { describe, it, expect } from 'vitest';
import { dayOf, daysFromNow, downloadsByDay, hours } from '@/components/admin/model';

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
});
