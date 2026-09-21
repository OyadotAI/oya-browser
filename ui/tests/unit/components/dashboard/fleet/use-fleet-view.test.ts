/**
 * Unit tests for the fleet table's view state and checkbox helpers.
 */
import { describe, it, expect } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { nextSort, useFleetView } from '@/components/dashboard/fleet/use-fleet-view';
import { toggled, useRowChecks } from '@/components/dashboard/fleet/use-row-checks';
import { PAGE_SIZE } from '@/components/dashboard/fleet/constants';
import { noFilter, row } from './fixtures';

describe('useFleetView', () => {
  it('flips the direction when the same column is clicked, and starts a new column ascending', () => {
    expect(nextSort({ key: 'name', dir: 1 }, 'name')).toEqual({ key: 'name', dir: -1 });
    expect(nextSort({ key: 'name', dir: -1 }, 'errors')).toEqual({ key: 'errors', dir: 1 });
  });

  it('paints one page of rows until asked for more', () => {
    const rows = Array.from({ length: PAGE_SIZE + 3 }, (_, i) => row({ id: `b${i}`, name: `n${i}` }));
    const { result } = renderHook(() => useFleetView(rows, noFilter));
    expect(result.current.shown).toHaveLength(PAGE_SIZE);
    act(() => result.current.setLimit(rows.length));
    expect(result.current.shown).toHaveLength(rows.length);
  });
});

describe('useRowChecks', () => {
  it('toggles one id without touching the original set', () => {
    const set = new Set(['a']);
    expect([...toggled(set, 'a')]).toEqual([]);
    expect([...toggled(set, 'b')]).toEqual(['a', 'b']);
    expect([...set]).toEqual(['a']);
  });

  it('selects every shown row, or clears them when all are checked', () => {
    const shown = [row({ id: 'a' }), row({ id: 'b' })];
    let next = new Set<string>();
    const none = renderHook(() => useRowChecks(shown, new Set(), (s) => (next = s)));
    none.result.current.toggleAll();
    expect([...next]).toEqual(['a', 'b']);
    const all = renderHook(() => useRowChecks(shown, new Set(['a', 'b']), (s) => (next = s)));
    expect(all.result.current.allChecked).toBe(true);
    all.result.current.toggleAll();
    expect(next.size).toBe(0);
  });
});
