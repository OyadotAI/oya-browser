/**
 * Unit tests for the profile screens' pure rules: caps, the drawer's dirty
 * check, row numbers, proxy labels and the check summary.
 */
import { describe, it, expect } from 'vitest';
import {
  capLabel,
  capText,
  capValue,
  checkSummary,
  choiceLabel,
  fieldsOf,
  isDirty,
  memberSince,
  rowStats,
} from '@/components/dashboard/personas/model';
import type { BrowserRow, Persona } from '@/components/dashboard/types';

const persona = {
  id: 'p1',
  name: 'ops',
  maxConcurrent: 2,
  activeBrowsers: 1,
  proxy: { geo: 'US' },
  exit: { id: 'x1', label: 'home', geo: 'US', healthy: true },
} as Persona;

describe('profile model', () => {
  it('reads a blank cap as uncapped both ways', () => {
    expect(capText(null)).toBe('');
    expect(capValue('')).toBeNull();
    expect(capValue('3')).toBe(3);
    expect(capLabel(null)).toBe('∞');
  });

  it('fills the drawer from what is stored, and is clean until something changes', () => {
    const f = fieldsOf(persona);
    expect(f).toEqual({ name: 'ops', cap: '2', geo: 'US', pin: 'x1' });
    expect(isDirty(f, persona)).toBe(false);
    expect(isDirty({ ...f, geo: 'DE' }, persona)).toBe(true);
  });

  it('does not count a changed pin as unsaved, because it applies on its own', () => {
    expect(isDirty({ ...fieldsOf(persona), pin: 'other' }, persona)).toBe(false);
  });

  it('counts running browsers from the fleet, falling back to the server count', () => {
    const browsers = [{ persona: 'p1' }, { persona: 'p1' }, { persona: 'p2' }] as BrowserRow[];
    expect(rowStats(persona, browsers)).toEqual({ cap: 2, at: false, running: 2 });
    expect(rowStats({ ...persona, maxConcurrent: null }, [])).toEqual({ cap: Infinity, at: false, running: 1 });
    expect(rowStats({ ...persona, activeBrowsers: 2 }, []).at).toBe(true);
  });

  it('labels a proxy choice with its geo, load and health', () => {
    const x = { id: 'a', label: 'home', geo: 'US', assigned: 1, maxPersonas: 2, available: false };
    expect(choiceLabel(x)).toBe('home · US · 1/2 · unhealthy');
    expect(choiceLabel({ ...x, geo: null, available: undefined })).toBe('home · 1/2');
  });

  it('summarises a proxy check as a failure when any proxy failed', () => {
    expect(
      checkSummary([
        { id: 'a', ok: true },
        { id: 'b', ok: false },
      ]),
    ).toEqual(['1 of 2 proxies failed', 'error']);
    expect(checkSummary([{ id: 'a', ok: true }])).toEqual(['All 1 proxies work', 'success']);
  });

  it('writes member-since only when the date is known', () => {
    expect(memberSince(undefined)).toBeNull();
    expect(memberSince('2026-01-15T12:00:00Z')).toContain('2026');
  });
});
