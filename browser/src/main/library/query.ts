/** Bounded, literal search over the current profile's local library. */
import type { LibraryEntry } from './library.ts';
import { QUERY_DEFAULT_LIMIT, QUERY_MAX_LIMIT, QUERY_MAX_LENGTH, BOOKMARK_TITLE_LENGTH } from './constants.ts';
/** Inputs remain unknown until checked at the desktop command boundary. */
export interface LibraryQuery {
  /** Case-insensitive substring in title or URL, not a regular expression. */
  query?: unknown;
  /** Maximum entries to return. */
  limit?: unknown;
  /** Number of matching entries to skip. */
  offset?: unknown;
}
/** Strict pagination avoids negative indexes and unbounded model responses. */
function pageNumber(value: unknown, fallback: number, minimum: number, maximum: number): number {
  if (value === undefined) return fallback;
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < minimum || value > maximum)
    throw new Error(`Pagination must be an integer between ${minimum} and ${maximum}`);
  return value;
}
/** Validate a search term instead of coercing arbitrary command input to text. */
function searchTerm(value: unknown): string {
  if (value === undefined) return '';
  if (typeof value !== 'string' || value.length > QUERY_MAX_LENGTH) throw new Error('Invalid library query');
  return value.trim().toLowerCase();
}
/** Results are newest first as stored, with an explicit cursor for the next batch. */
export function queryLibrary(entries: LibraryEntry[], args: LibraryQuery = {}) {
  const query = searchTerm(args.query);
  const limit = pageNumber(args.limit, QUERY_DEFAULT_LIMIT, 1, QUERY_MAX_LIMIT);
  const offset = pageNumber(args.offset, 0, 0, Number.MAX_SAFE_INTEGER);
  const matches = entries.filter((entry) => `${entry.title} ${entry.url}`.toLowerCase().includes(query));
  return queryPage(matches, offset, limit);
}
/** Keep title size and result count bounded while preserving full navigation targets. */
function queryPage(matches: LibraryEntry[], offset: number, limit: number) {
  const entries = matches.slice(offset, offset + limit).map((entry) => ({
    ...entry,
    title: entry.title.slice(0, BOOKMARK_TITLE_LENGTH),
  }));
  const next = offset + entries.length;
  return { entries, total: matches.length, next_offset: next < matches.length ? next : null };
}
