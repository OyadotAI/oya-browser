/**
 * Unit tests for reading another browser's bookmarks and history from its files:
 * a Chromium profile's Bookmarks JSON and History database, and Firefox's
 * places.sqlite, built here with the system's sqlite3 as those browsers lay them out.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { readLibrary, readLibraryDetailed } from '../../../../src/main/mirror/library-read.ts';

/** 2026-01-01T00:00:00Z in epoch milliseconds. */
const AT = Date.UTC(2026, 0, 1);
/** The same moment in Chromium's microseconds since 1601. */
const WEBKIT = (AT + 11_644_473_600_000) * 1000;

/** A fresh directory with `sql` run into the SQLite file `name` inside `sub`. */
function profileWith(sub: string, name: string, sql: string): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-libtest-'));
  fs.mkdirSync(path.join(dir, sub), { recursive: true });
  execFileSync('sqlite3', [path.join(dir, sub, name), sql]);
  return dir;
}

it("reads a Chromium profile's bookmarks from every folder, and its visits newest first", () => {
  const dir = profileWith(
    'Default',
    'History',
    `CREATE TABLE urls(url, title, last_visit_time, hidden);
     INSERT INTO urls VALUES ('https://old.test/', 'Old', ${WEBKIT - 1000}, 0), ('https://new.test/', NULL, ${WEBKIT}, 0), ('https://hid.test/', 'H', ${WEBKIT}, 1);`,
  );
  const folder = {
    type: 'folder',
    children: [{ type: 'url', name: 'Deep', url: 'https://deep.test/', date_added: String(WEBKIT) }],
  };
  const roots = { bookmark_bar: { type: 'folder', children: [folder] }, other: { type: 'folder', children: [] } };
  fs.writeFileSync(path.join(dir, 'Default', 'Bookmarks'), JSON.stringify({ roots }));
  const library = readLibrary('chrome', dir, 'Default');
  assert.deepEqual(library.bookmarks, [{ url: 'https://deep.test/', title: 'Deep', time: AT }]);
  assert.deepEqual(
    library.history.map((e) => [e.url, e.title]),
    [
      ['https://new.test/', 'https://new.test/'],
      ['https://old.test/', 'Old'],
    ],
  );
  assert.equal(library.history[0].time, AT);
});

it("reads Firefox's bookmarks and visits from places.sqlite, through profiles.ini", () => {
  const dir = profileWith(
    'abc.default-release',
    'places.sqlite',
    `CREATE TABLE moz_places(id, url, title, last_visit_date, hidden);
     CREATE TABLE moz_bookmarks(id, type, fk, title, dateAdded);
     INSERT INTO moz_places VALUES (1, 'https://fx.test/', 'Fx', ${AT * 1000}, 0), (2, 'https://never.test/', 'N', NULL, 0);
     INSERT INTO moz_bookmarks VALUES (1, 1, 1, 'Saved', ${AT * 1000}), (2, 2, NULL, 'Folder', 0);`,
  );
  fs.writeFileSync(path.join(dir, 'profiles.ini'), '[Profile0]\nPath=abc.default-release\nDefault=1\n');
  const library = readLibrary('firefox', dir, 'default');
  assert.deepEqual(library.bookmarks, [{ url: 'https://fx.test/', title: 'Saved', time: AT }]);
  assert.deepEqual(library.history, [{ url: 'https://fx.test/', title: 'Fx', time: AT }]);
});

it("reads Safari's bookmarks without the reading list, and its visits from History.db", (t) => {
  const dir = profileWith(
    '.',
    'History.db',
    `CREATE TABLE history_items(id, url); CREATE TABLE history_visits(history_item, visit_time, title);
     INSERT INTO history_items VALUES (1, 'https://sa.test/');
     INSERT INTO history_visits VALUES (1, ${AT / 1000 - 978_307_200 - 60}, 'Earlier'), (1, ${AT / 1000 - 978_307_200}, 'Latest');`,
  );
  const leaf = (url: string, title: string) => ({
    WebBookmarkType: 'WebBookmarkTypeLeaf',
    URLString: url,
    URIDictionary: { title },
  });
  const tree = {
    Children: [
      { Title: 'BookmarksBar', Children: [{ Children: [leaf('https://bar.test/', 'Bar')] }] },
      { Title: 'com.apple.ReadingList', Children: [leaf('https://later.test/', 'Later')] },
    ],
  };
  const json = path.join(dir, 'Bookmarks.json');
  fs.writeFileSync(json, JSON.stringify(tree));
  try {
    execFileSync('plutil', ['-convert', 'binary1', '-o', path.join(dir, 'Bookmarks.plist'), json], { stdio: 'ignore' });
  } catch {
    return t.skip('Safari is read on macOS only: no plutil here');
  }
  const library = readLibrary('safari', dir, 'default');
  assert.deepEqual(
    library.bookmarks.map((e) => [e.url, e.title]),
    [['https://bar.test/', 'Bar']],
  );
  assert.deepEqual(library.history, [{ url: 'https://sa.test/', title: 'Latest', time: AT }]);
});

it('reads nothing, without failing, from a profile with no bookmarks or history files', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-libtest-'));
  assert.deepEqual(readLibrary('chrome', dir, 'Default'), { bookmarks: [], history: [] });
  assert.deepEqual(readLibrary('firefox', dir, 'default'), { bookmarks: [], history: [] });
});

it('a corrupt history database preserves readable bookmarks and reports only that failed part', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-partial-library-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'Default'));
  fs.writeFileSync(
    path.join(root, 'Default', 'Bookmarks'),
    JSON.stringify({
      roots: {
        bookmark_bar: { type: 'url', url: 'https://example.com/', name: 'Example', date_added: String(WEBKIT) },
      },
    }),
  );
  fs.writeFileSync(path.join(root, 'Default', 'History'), 'not a database');
  const result = readLibraryDetailed(root, 'Default');
  assert.equal(result.bookmarks.length, 1);
  assert.deepEqual(result.history, []);
  assert.match(result.warnings.join(' '), /History could not be read/);
  assert(!result.warnings.join(' ').includes(root));
});
it('an empty profile is distinguishable from unreadable content and linked files are refused', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-empty-library-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'Default'));
  assert.deepEqual(readLibraryDetailed(root, 'Default'), { bookmarks: [], history: [], warnings: [] });
  fs.writeFileSync(path.join(root, 'outside'), '{}');
  fs.symlinkSync(path.join(root, 'outside'), path.join(root, 'Default', 'Bookmarks'));
  assert.match(readLibraryDetailed(root, 'Default').warnings.join(' '), /Bookmarks could not be read/);
});
