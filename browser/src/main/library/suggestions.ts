/** Private, bounded address completion using the active persona's existing library. */
import type { AddressSuggestion } from '../../shared/ipc.ts';
import type { BrowsingLibrary, LibraryEntry } from './library.ts';
import { ADDRESS_SUGGESTION_LIMIT, BOOKMARK_TITLE_LENGTH, QUERY_MAX_LENGTH } from './constants.ts';

/** Prefer host/path prefixes, then the recent order already maintained by the library. */
function prefix(url: string, query: string): boolean {
  return url
    .toLowerCase()
    .replace(/^https?:\/\/(www\.)?/, '')
    .startsWith(query);
}
/** A title is text, not HTML, and need not expose an unbounded page-provided string. */
function suggestion(entry: LibraryEntry, source: AddressSuggestion['source']): AddressSuggestion {
  return { url: entry.url, title: entry.title.slice(0, BOOKMARK_TITLE_LENGTH), source };
}
/** Only local HTTP(S) entries are returned, bookmarks winning duplicate URLs. */
export function addressSuggestions(library: BrowsingLibrary, value: unknown): AddressSuggestion[] {
  if (typeof value !== 'string' || value.length > QUERY_MAX_LENGTH) return [];
  const query = value.trim().toLowerCase();
  if (!query) return [];
  const unique = libraryEntries(library);
  return unique
    .filter((e) => `${e.title} ${e.url}`.toLowerCase().includes(query))
    .sort((a, b) => Number(prefix(b.url, query)) - Number(prefix(a.url, query)))
    .slice(0, ADDRESS_SUGGESTION_LIMIT);
}

/** Preserve bookmark preference and recency while returning each address only once. */
function deduplicate(entries: AddressSuggestion[]): AddressSuggestion[] {
  const seen = new Map<string, AddressSuggestion>();
  for (const entry of entries) if (!seen.has(entry.url)) seen.set(entry.url, entry);
  return [...seen.values()];
}

/** Bookmarks win duplicate addresses without displacing newer history-only entries. */
function libraryEntries(library: BrowsingLibrary): AddressSuggestion[] {
  const { bookmarks, history } = library.snapshot();
  return deduplicate([
    ...bookmarks.map((e) => suggestion(e, 'bookmark')),
    ...history.map((e) => suggestion(e, 'history')),
  ]);
}
