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
interface LibraryData {
  /** Recently visited pages, newest first, deduplicated by address. */
  history: LibraryEntry[];
  /** Explicitly saved pages, newest first. */
  bookmarks: LibraryEntry[];
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
/** A library resolves its partition on every operation, including after a persona switch. */
export class BrowsingLibrary {
  /** Persistence and the current persona. */
  private readonly deps: Deps;
  /** Uses the same repository as desktop settings. */
  constructor(deps: Deps) {
    this.deps = deps;
  }
  /** Returns only the current persona's data. */
  snapshot(): LibraryData {
    const all = this.deps.config.values.browsingLibrary as Record<string, unknown> | undefined;
    return data(all?.[this.deps.persona.partitionName()]);
  }
  /** Writes only this persona while retaining other personas' libraries. */
  private save(value: LibraryData): void {
    const all = this.deps.config.values.browsingLibrary as Record<string, unknown> | undefined;
    this.deps.config.merge({ browsingLibrary: { ...all, [this.deps.persona.partitionName()]: value } });
    this.deps.config.save();
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
