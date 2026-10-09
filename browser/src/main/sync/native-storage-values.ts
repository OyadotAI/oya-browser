/** Validate complete imports before mutation and native snapshots before sending them off the browser. */
import type { Origins } from '../../page/login-state.ts';
import { STORAGE_ENTRIES_MAX, STORAGE_ORIGINS_MAX, STORAGE_UNITS_MAX, STORAGE_PAIR_LENGTH } from './constants.ts';
/** Canonical origins are capabilities, not arbitrary URLs or opaque frame identities. */
export function storageOrigin(origin: string): string {
  const parsed = URL.parse(origin);
  if (!parsed || !['http:', 'https:'].includes(parsed.protocol) || parsed.origin !== origin)
    throw Error('Invalid native storage origin');
  return origin;
}
/** Arrays of pairs preserve special keys without prototype assignment or UTF-8 conversion. */
export function storageValues(entries: string[][]): Record<string, string> {
  const keys = new Set<string>();
  if (!Array.isArray(entries) || entries.length > STORAGE_ENTRIES_MAX) throw Error('Invalid native storage snapshot');
  entries.forEach((entry) => validateEntry(entry, keys));
  const units = entries.reduce((size, [key, value]) => size + key.length + value.length, 0);
  if (units > STORAGE_UNITS_MAX) throw Error('Native storage snapshot exceeds limit');
  return Object.fromEntries(entries);
}
/** Reject duplicates and malformed pairs rather than silently discarding or coercing data. */
function validateEntry(entry: string[], keys: Set<string>): void {
  if (
    !Array.isArray(entry) ||
    entry.length !== STORAGE_PAIR_LENGTH ||
    entry.some((value) => typeof value !== 'string') ||
    keys.has(entry[0])
  )
    throw Error('Invalid native storage entry');
  keys.add(entry[0]);
}
/** Validate every origin and value before the first native write occurs. */
export function storageImport(origins: Origins): [string, string[][]][] {
  if (!origins || typeof origins !== 'object' || Array.isArray(origins)) throw Error('Invalid native storage import');
  const entries = Object.entries(origins);
  if (entries.length > STORAGE_ORIGINS_MAX) throw Error('Too many native storage origins');
  return entries.map(([origin, values]) => importOrigin(origin, values));
}
/** Import objects come from profile JSON; arrays and null are not storage dictionaries. */
function importOrigin(origin: string, values: Record<string, string>): [string, string[][]] {
  storageOrigin(origin);
  if (!values || typeof values !== 'object' || Array.isArray(values)) throw Error('Invalid native storage values');
  const entries = Object.entries(values);
  storageValues(entries);
  return [origin, entries];
}
