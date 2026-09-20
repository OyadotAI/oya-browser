/**
 * Which browsers the fleet table shows, and in what order: the filter and the
 * sort, as pure functions so they can be tested without rendering.
 */
import type { BrowserRow } from '../types';
import type { FleetFilter } from '../fleet-strip';
import { HEALTH_RANK, type SortKey } from './constants';

/** The current sort: a column and a direction (1 ascending, -1 descending). */
export interface Sort {
  /** Column sorted by. */
  key: SortKey;
  /** 1 ascending, -1 descending. */
  dir: 1 | -1;
}

/** The persona a row is grouped under in the strip, as the persona filter compares it. */
const personaKey = (r: BrowserRow) => r.personaName || r.persona || '—';

/** Whether the free-text filter appears in any of the row's searchable fields. */
function matchesText(r: BrowserRow, text: string): boolean {
  const q = text.toLowerCase();
  return [r.name, r.id, r.currentUrl, r.personaName, r.persona, r.provider].some((v) =>
    (v || '').toLowerCase().includes(q),
  );
}

/** Whether a row passes every active filter. */
export function matches(r: BrowserRow, f: FleetFilter): boolean {
  if (f.health && r.health !== f.health) return false;
  if (f.provider && r.provider !== f.provider) return false;
  if (f.persona && personaKey(r) !== f.persona) return false;
  return !f.text || matchesText(r, f.text);
}

/** How a column's value is read for sorting, where it is not the raw field. */
const SORT_VALUE: Partial<Record<SortKey, (r: BrowserRow) => string | number>> = {
  health: (r) => HEALTH_RANK[r.health],
  persona: (r) => r.personaName || r.persona || '',
  commands: (r) => r.commands,
  errors: (r) => r.errors,
  lastSeen: (r) => r.lastSeen,
  connectedAt: (r) => r.connectedAt,
};

/** A row's value for the sort column; text columns treat a missing value as empty. */
function sortValue(r: BrowserRow, key: SortKey): string | number {
  if (Object.hasOwn(SORT_VALUE, key)) return SORT_VALUE[key]!(r);
  return (r[key as keyof BrowserRow] || '') as string;
}

/** Orders two rows by the sort, then by name so equal rows keep a stable order. */
function compare(a: BrowserRow, b: BrowserRow, { key, dir }: Sort): number {
  const av = sortValue(a, key);
  const bv = sortValue(b, key);
  const c = av < bv ? -1 : av > bv ? 1 : 0;
  return c * dir || a.name.localeCompare(b.name);
}

/** The rows that pass the filter, sorted. */
export function visibleRows(rows: BrowserRow[], filter: FleetFilter, sort: Sort): BrowserRow[] {
  return rows.filter((r) => matches(r, filter)).sort((a, b) => compare(a, b, sort));
}

/** Whether any filter is set, so the toolbar offers to clear them. */
export const anyFilter = (f: FleetFilter) => !!(f.health || f.provider || f.persona || f.text);

/** Entries of a count table, largest first. */
export const byCount = (counts: Record<string, number> | undefined) =>
  Object.entries(counts || {}).sort((x, y) => y[1] - x[1]);
