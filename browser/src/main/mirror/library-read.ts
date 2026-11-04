/**
 * Bookmarks and history from the browser an import reads, straight from its files:
 * Chromium's Bookmarks JSON and History database, Firefox's places.sqlite, Safari's
 * Bookmarks.plist and History.db. Read from
 * copies (sqlite.ts), so a running browser's lock never matters. A part that cannot be read is
 * left empty rather than failing the logins it came with.
 */
import fs from 'node:fs';
import path from 'node:path';
import { sqliteRows } from './sqlite.ts';
import { firefoxProfile } from './firefox.ts';
import { plistJson } from './safari.ts';
import type { LibraryEntry } from '../library/index.ts';
import {
  IMPORTED_HISTORY_LIMIT,
  MAC_EPOCH_OFFSET_S,
  MS_PER_SECOND,
  US_PER_MS,
  WEBKIT_EPOCH_OFFSET_MS,
} from './constants.ts';

/** What an import brings for the library. */
export interface ImportedLibrary {
  /** Saved pages. */
  bookmarks: LibraryEntry[];
  /** Visited pages, newest first. */
  history: LibraryEntry[];
}

/** A partial import distinguishes absent content from an unreadable part of the profile. */
export interface LibraryReadResult extends ImportedLibrary {
  /** Safe part-specific failures without local paths or private content. */
  warnings: string[];
}

/** A node of Chromium's Bookmarks file: a folder with children, or a URL. */
interface ChromiumNode {
  /** 'url' or 'folder'. */
  type?: string;
  /** The title. */
  name?: string;
  /** The address, on a URL. */
  url?: string;
  /** Microseconds since 1601, as text. */
  date_added?: string;
  /** A folder's contents. */
  children?: ChromiumNode[];
}

/** A row the history and bookmark queries answer with. */
interface Row {
  /** The address. */
  url: string;
  /** The title, which may be missing. */
  title: string | null;
  /** Microseconds, since 1601 (Chromium) or 1970 (Firefox). */
  time: number;
}

/** Chromium's most recent visits. */
const CHROMIUM_HISTORY = `SELECT url, title, last_visit_time AS time FROM urls WHERE hidden = 0 ORDER BY last_visit_time DESC LIMIT ${IMPORTED_HISTORY_LIMIT}`;
/** Firefox's most recent visits. */
const FIREFOX_HISTORY = `SELECT url, title, last_visit_date AS time FROM moz_places WHERE last_visit_date IS NOT NULL AND hidden = 0 ORDER BY last_visit_date DESC LIMIT ${IMPORTED_HISTORY_LIMIT}`;
/** Safari's most recent visits, one per address, in seconds since 2001. */
const SAFARI_HISTORY = `SELECT i.url AS url, v.title AS title, MAX(v.visit_time) AS time FROM history_items i JOIN history_visits v ON v.history_item = i.id GROUP BY i.id ORDER BY time DESC LIMIT ${IMPORTED_HISTORY_LIMIT}`;
/** Firefox's bookmarks (type 1 is a URL; folders and separators are left out). */
const FIREFOX_BOOKMARKS =
  'SELECT p.url AS url, b.title AS title, b.dateAdded AS time FROM moz_bookmarks b JOIN moz_places p ON p.id = b.fk WHERE b.type = 1 ORDER BY b.dateAdded DESC';

/** A Chromium time (microseconds since 1601) in epoch milliseconds. */
const webkitMs = (us: number | string | undefined): number => Number(us || 0) / US_PER_MS - WEBKIT_EPOCH_OFFSET_MS;
/** A Firefox time (microseconds since 1970) in epoch milliseconds. */
const unixMs = (us: number): number => Number(us || 0) / US_PER_MS;

/** A Safari time (seconds since 2001) in epoch milliseconds. */
const macMs = (s: number): number => (Number(s || 0) + MAC_EPOCH_OFFSET_S) * MS_PER_SECOND;

/** A query row as a library entry, its time converted by `ms`. */
const entryOf =
  (ms: (us: number) => number) =>
  (row: Row): LibraryEntry => ({
    url: row.url,
    title: row.title || row.url,
    time: ms(row.time),
  });

/** Every URL bookmark under a Chromium node, its folders flattened. */
function chromiumBookmarks(node: ChromiumNode): LibraryEntry[] {
  if (node?.type === 'url')
    return [{ url: node.url ?? '', title: node.name || node.url || '', time: webkitMs(node.date_added) }];
  return (node?.children ?? []).flatMap(chromiumBookmarks);
}

/** The rows `sql` answers from a copy of a SQLite file. */
const query = (file: string, sql: string): Row[] => sqliteRows<Row>(file, sql);

/** `read()`, or nothing when the file is missing, locked away or not what was expected. */
function tolerant(read: () => LibraryEntry[]): LibraryEntry[] {
  try {
    return read();
  } catch {
    return [];
  }
}

/** A Chromium profile's bookmarks and history. */
function chromiumLibrary(profileDir: string): ImportedLibrary {
  const file = (name: string) => path.join(profileDir, name);
  const roots = () => Object.values(JSON.parse(fs.readFileSync(file('Bookmarks'), 'utf8')).roots ?? {});
  return {
    bookmarks: tolerant(() => roots().flatMap((root) => chromiumBookmarks(root as ChromiumNode))),
    history: tolerant(() => query(file('History'), CHROMIUM_HISTORY).map(entryOf(webkitMs))),
  };
}

/** Reads one part without allowing file symlinks to escape the selected profile. */
function libraryPart(file: string, label: string, read: () => LibraryEntry[], warnings: string[]): LibraryEntry[] {
  try {
    requireSourceFile(file);
    return read();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return [];
    warnings.push(`${label} could not be read. Check this profile’s file permissions and try again.`);
    return [];
  }
}

/** Offline reads refuse links and directories instead of following them outside the selected profile. */
function requireSourceFile(file: string): void {
  const info = fs.lstatSync(file);
  if (info.isSymbolicLink() || !info.isFile()) throw Error('Unsupported source file');
}

/** Chromium's readable library is independent of its encrypted cookies and executable. */
export function readLibraryDetailed(userDataDir: string, profile: string): LibraryReadResult {
  const folder = path.join(userDataDir, profile);
  const [bookmarksFile, historyFile] = ['Bookmarks', 'History'].map((name) => path.join(folder, name));
  const warnings: string[] = [];
  const bookmarks = libraryPart(bookmarksFile, 'Bookmarks', () => readChromiumBookmarks(bookmarksFile), warnings);
  const history = libraryPart(historyFile, 'History', () => readChromiumHistory(historyFile), warnings);
  return { bookmarks, history, warnings };
}

/** Flatten the selected profile's saved folders without exposing unreadable file details. */
function readChromiumBookmarks(file: string): LibraryEntry[] {
  return Object.values(JSON.parse(fs.readFileSync(file, 'utf8')).roots ?? {}).flatMap((node) =>
    chromiumBookmarks(node as ChromiumNode),
  );
}
/** Recent visits use the same bound and timestamp conversion as existing imports. */
const readChromiumHistory = (file: string): LibraryEntry[] => query(file, CHROMIUM_HISTORY).map(entryOf(webkitMs));

/** A node of Safari's Bookmarks.plist: a list with children, or a leaf with an address. */
interface SafariNode {
  /** WebBookmarkTypeList or WebBookmarkTypeLeaf. */
  WebBookmarkType?: string;
  /** A list's name; the reading list is com.apple.ReadingList. */
  Title?: string;
  /** A leaf's address. */
  URLString?: string;
  /** A leaf's title. */
  URIDictionary?: { /** The page's title. */ title?: string };
  /** A list's contents. */
  Children?: SafariNode[];
}

/** Every bookmark under a Safari node, folders flattened, the reading list left out; stamped `time`. */
function safariBookmarks(node: SafariNode, time: number): LibraryEntry[] {
  if (node?.WebBookmarkType === 'WebBookmarkTypeLeaf' && node.URLString)
    return [{ url: node.URLString, title: node.URIDictionary?.title || node.URLString, time }];
  if (node?.Title === 'com.apple.ReadingList') return [];
  return (node?.Children ?? []).flatMap((child) => safariBookmarks(child, time));
}

/** Safari's bookmarks and history. */
function safariLibrary(dir: string): ImportedLibrary {
  return {
    bookmarks: tolerant(() => safariBookmarks(plistJson(path.join(dir, 'Bookmarks.plist')) as SafariNode, Date.now())),
    history: tolerant(() => query(path.join(dir, 'History.db'), SAFARI_HISTORY).map(entryOf(macMs))),
  };
}

/** A Firefox profile's bookmarks and history. */
function firefoxLibrary(profileDir: string): ImportedLibrary {
  const places = path.join(profileDir, 'places.sqlite');
  return {
    bookmarks: tolerant(() => query(places, FIREFOX_BOOKMARKS).map(entryOf(unixMs))),
    history: tolerant(() => query(places, FIREFOX_HISTORY).map(entryOf(unixMs))),
  };
}

/** The bookmarks and history of profile `profile` of the browser at `userDataDir`. */
export function readLibrary(source: string, userDataDir: string, profile: string): ImportedLibrary {
  if (source === 'safari') return safariLibrary(userDataDir);
  if (source !== 'firefox') return chromiumLibrary(path.join(userDataDir, profile));
  const profileDir = firefoxProfile(userDataDir);
  return profileDir ? firefoxLibrary(profileDir) : { bookmarks: [], history: [] };
}
