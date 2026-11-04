/** Local history and bookmarks, persisted through the settings repository, never synced. */
import type { AppServices } from '../app/services.ts';
import { queryLibrary, type LibraryQuery } from './query.ts';
import { HISTORY_LIMIT, BOOKMARK_TITLE_LENGTH } from './constants.ts';

/** A saved page and when it was visited or bookmarked. */
export interface LibraryEntry {
  /** Safe web address to reopen. */
  url: string;
  /** Page title, falling back to the address. */
  title: string;
  /** Milliseconds since the epoch. */
  time: number;
}
/** The library of a single persona partition. */
export interface LibraryData {
  /** Recently visited pages, newest first, deduplicated by address. */
  history: LibraryEntry[];
  /** Explicitly saved pages, newest first. */
  bookmarks: LibraryEntry[];
}
/** Counts measure additions retained by the current partition, rather than raw source rows. */
interface ImportCounts {
  /** New valid, unique bookmarks. */
  bookmarks: number;
  /** New history addresses retained under the history cap. */
  history: number;
}
/** Compute additions after the same filtering and bounds that persistence uses. */
function addedCounts(current: LibraryData, next: LibraryData): ImportCounts {
  const known = new Set(current.history.map((entry) => entry.url));
  return {
    bookmarks: next.bookmarks.length - current.bookmarks.length,
    history: next.history.filter((entry) => !known.has(entry.url)).length,
  };
}
/** Dependencies keep persistence and profile identity outside the service. */
type Deps = Pick<AppServices, 'config' | 'persona'>;

/** Only ordinary web pages belong in a library; never store embedded credentials. */
export function safeAddress(value: string): boolean {
  try {
    const url = new URL(value);
    return ['https:', 'http:'].includes(url.protocol) && !url.username && !url.password;
  } catch {
    return false;
  }
}
/** Agent bookmark writes reject bad addresses rather than silently reporting success. */
function bookmarkEntry(url: unknown, title: unknown): LibraryEntry {
  if (typeof url !== 'string' || !safeAddress(url)) throw new Error('Bookmark URL must be HTTP(S) without credentials');
  if (title !== undefined && (typeof title !== 'string' || title.length > BOOKMARK_TITLE_LENGTH))
    throw new Error('Invalid bookmark title');
  return { url, title: typeof title === 'string' && title ? title : url, time: Date.now() };
}
/** An entry must carry a safe URL and a usable title and timestamp. */
function validEntry(entry: LibraryEntry | null): entry is LibraryEntry {
  return (
    !!entry &&
    typeof entry.url === 'string' &&
    safeAddress(entry.url) &&
    typeof entry.title === 'string' &&
    Number.isFinite(entry.time)
  );
}
/** Discards malformed saved entries without compromising the rest of the library. */
function entries(value: unknown): LibraryEntry[] {
  return Array.isArray(value) ? value.filter(validEntry) : [];
}
/** Reading from disk does not trust obsolete or malformed settings. */
function data(value: unknown): LibraryData {
  const saved = value as Partial<LibraryData> | null;
  return { history: entries(saved?.history).slice(0, HISTORY_LIMIT), bookmarks: entries(saved?.bookmarks) };
}
/** `imported` added to `current`: the bookmarks it lacked after its own, and every visit by address, newest first. */
function merged(current: LibraryData, imported: LibraryData): LibraryData {
  const saved = new Set(current.bookmarks.map((entry) => entry.url));
  const bookmarks = [...current.bookmarks, ...uniqueNewBookmarks(entries(imported.bookmarks), saved)];
  const history = newestByAddress([...current.history, ...entries(imported.history)]).slice(0, HISTORY_LIMIT);
  return { bookmarks, history };
}

/** Imports are idempotent even when the source repeats a bookmark in several folders. */
function uniqueNewBookmarks(imported: LibraryEntry[], saved: Set<string>): LibraryEntry[] {
  return imported.filter((entry) => {
    if (saved.has(entry.url)) return false;
    saved.add(entry.url);
    return true;
  });
}
/** One entry per address, the latest visit, newest first. */
function newestByAddress(list: LibraryEntry[]): LibraryEntry[] {
  const latest = new Map<string, LibraryEntry>();
  for (const entry of list) if ((latest.get(entry.url)?.time ?? -Infinity) < entry.time) latest.set(entry.url, entry);
  return [...latest.values()].sort((a, b) => b.time - a.time);
}
/** A library resolves its partition on every operation, including after a persona switch. */
export class BrowsingLibrary {
  /** Persistence and the current persona. */
  private readonly deps: Deps;
  /** Hears every change, so the bookmarks bar follows it. */
  changed: () => void = () => {};
  /** Uses the same repository as desktop settings. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Returns only the current persona's data, or the persona whose partition is `partition`. */
  snapshot(partition = this.deps.persona.partitionName()): LibraryData {
    const all = this.deps.config.values.browsingLibrary as Record<string, unknown> | undefined;
    return data(all?.[partition]);
  }
  /** Writes only this persona (or the one at `partition`) while retaining other personas' libraries. */
  private save(value: LibraryData, partition = this.deps.persona.partitionName()): void {
    const all = this.deps.config.values.browsingLibrary as Record<string, unknown> | undefined;
    this.deps.config.merge({ browsingLibrary: { ...all, [partition]: value } });
    this.deps.config.save();
    this.changed();
  }
  /** Adds another browser's bookmarks and history to the persona at `partition`, keeping what it has. */
  importInto(partition: string, imported: LibraryData): ImportCounts {
    const current = this.snapshot(partition);
    const next = merged(current, imported);
    const counts = addedCounts(current, next);
    this.save(next, partition);
    return counts;
  }
  /** Records a committed main-frame page; repeat visits move it to the front. */
  visit(url: string, title: string): void {
    if (!safeAddress(url)) return;
    const current = this.snapshot();
    const history = [{ url, title: title || url, time: Date.now() }, ...current.history.filter((e) => e.url !== url)];
    this.save({ ...current, history: history.slice(0, HISTORY_LIMIT) });
  }
  /** Adds the page, or removes it if it is already bookmarked. */
  toggle(url: string, title: string): void {
    if (!safeAddress(url)) return;
    const current = this.snapshot();
    const bookmarks = current.bookmarks.filter((entry) => entry.url !== url);
    if (bookmarks.length === current.bookmarks.length)
      bookmarks.unshift({ url, title: title || url, time: Date.now() });
    this.save({ ...current, bookmarks });
  }
  /** Search only the current partition, with bounded results and pagination. */
  search(kind: 'history' | 'bookmarks', args: LibraryQuery = {}) {
    return queryLibrary(this.snapshot()[kind], args);
  }
  /** Idempotently saves a bookmark; retries never toggle it off. */
  addBookmark(url: unknown, title?: unknown): LibraryEntry {
    const entry = bookmarkEntry(url, title);
    const current = this.snapshot();
    const prior = current.bookmarks.find((item) => item.url === entry.url);
    const saved = prior && title === undefined ? prior : entry;
    this.save({ ...current, bookmarks: [saved, ...current.bookmarks.filter((item) => item.url !== entry.url)] });
    return saved;
  }
  /** Removes exactly one URL; retrying an already removed bookmark is harmless. */
  removeBookmark(url: unknown): boolean {
    const entry = bookmarkEntry(url, undefined);
    const current = this.snapshot();
    const bookmarks = current.bookmarks.filter((item) => item.url !== entry.url);
    if (bookmarks.length === current.bookmarks.length) return false;
    this.save({ ...current, bookmarks });
    return true;
  }
  /** Explicitly forgets history, leaving saved bookmarks intact. */
  clearHistory(): void {
    this.save({ ...this.snapshot(), history: [] });
  }
}
