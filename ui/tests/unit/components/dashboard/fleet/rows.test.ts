/**
 * Unit tests for the fleet table's filter and sort.
 */
import { describe, it, expect } from 'vitest';
import { anyFilter, byCount, matches, visibleRows } from '@/components/dashboard/fleet/rows';
import { noFilter, row } from './fixtures';

describe('fleet rows', () => {
  it('filters by health, provider and persona', () => {
    const r = row({ health: 'errors', provider: 'steel', personaName: 'Sales' });
    expect(matches(r, { ...noFilter, health: 'errors', provider: 'steel', persona: 'Sales' })).toBe(true);
    expect(matches(r, { ...noFilter, health: 'ok' })).toBe(false);
    expect(matches(r, { ...noFilter, provider: 'anchor' })).toBe(false);
  });

  it('groups a browser without a persona under "—"', () => {
    expect(matches(row(), { ...noFilter, persona: '—' })).toBe(true);
  });

  it('matches free text case-insensitively across name, id, url, persona and provider', () => {
    expect(matches(row({ currentUrl: 'https://Shop.test' }), { ...noFilter, text: 'shop' })).toBe(true);
    expect(matches(row(), { ...noFilter, text: 'nowhere' })).toBe(false);
  });

  it('sorts browsers needing attention first, then by name', () => {
    const rows = [
      row({ id: 'a', name: 'b', health: 'ok' }),
      row({ id: 'b', name: 'a', health: 'ok' }),
      row({ id: 'c', name: 'z', health: 'errors' }),
    ];
    expect(visibleRows(rows, noFilter, { key: 'health', dir: 1 }).map((r) => r.id)).toEqual(['c', 'b', 'a']);
  });

  it('reverses the order when descending', () => {
    const rows = [row({ id: 'a', commands: 1 }), row({ id: 'b', commands: 5 })];
    expect(visibleRows(rows, noFilter, { key: 'commands', dir: -1 }).map((r) => r.id)).toEqual(['b', 'a']);
  });

  it('sorts text columns with missing values as empty', () => {
    const rows = [row({ id: 'a', provider: 'steel' }), row({ id: 'b', provider: null })];
    expect(visibleRows(rows, noFilter, { key: 'provider', dir: 1 }).map((r) => r.id)).toEqual(['b', 'a']);
  });

  it('reports whether any filter is set', () => {
    expect(anyFilter(noFilter)).toBe(false);
    expect(anyFilter({ ...noFilter, text: 'x' })).toBe(true);
  });

  it('lists counts largest first', () => {
    expect(byCount({ a: 1, b: 3 })).toEqual([
      ['b', 3],
      ['a', 1],
    ]);
    expect(byCount(undefined)).toEqual([]);
  });
});
