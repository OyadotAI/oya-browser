/** Behavior of the persistent, profile-separated browsing library. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BrowsingLibrary } from '../../../../src/main/library/index.ts';
import { HISTORY_LIMIT } from '../../../../src/main/library/constants.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** A library backed by an in-memory settings repository. */
function setup() {
  const ctx = mainCtx();
  let partition = 'first';
  ctx.persona.partitionName = () => partition;
  return {
    ctx,
    library: new BrowsingLibrary(ctx),
    switchTo: (value) => {
      partition = value;
    },
  };
}
describe('BrowsingLibrary', () => {
  it('adds and removes bookmarks idempotently, with optional title updates', () => {
    const { library } = setup();
    library.addBookmark('https://a.test/', 'A');
    library.addBookmark('https://a.test/');
    assert.equal(library.snapshot().bookmarks.length, 1);
    assert.equal(library.snapshot().bookmarks[0].title, 'A');
    library.addBookmark('https://a.test/', 'Renamed');
    assert.equal(library.snapshot().bookmarks[0].title, 'Renamed');
    assert.equal(library.removeBookmark('https://a.test/'), true);
    assert.equal(library.removeBookmark('https://a.test/'), false);
  });
  it('rejects unsafe writes and searches only the current profile', () => {
    const { library, switchTo } = setup();
    for (const url of [undefined, 'file:///private', 'javascript:alert(1)', 'https://user:secret@a.test/']) {
      assert.throws(() => library.addBookmark(url));
      assert.throws(() => library.removeBookmark(url));
    }
    assert.throws(() => library.addBookmark('https://a.test/', 42));
    library.addBookmark('https://a.test/');
    assert.equal(library.search('bookmarks', { query: 'a.test' }).total, 1);
    switchTo('other');
    assert.equal(library.search('bookmarks').total, 0);
  });

  it('remembers a visit across service recreation', () => {
    const { ctx, library } = setup();
    library.visit('https://a.test/', 'A');
    assert.equal(new BrowsingLibrary(ctx).snapshot().history[0].title, 'A');
  });
  it('deduplicates repeat visits and places the newest first', () => {
    const { library } = setup();
    library.visit('https://a.test/', 'A');
    library.visit('https://b.test/', 'B');
    library.visit('https://a.test/', 'Updated A');
    assert.deepEqual(
      library.snapshot().history.map((entry) => entry.title),
      ['Updated A', 'B'],
    );
  });
  it('keeps history and bookmarks isolated when switching personas', () => {
    const { library, switchTo } = setup();
    library.visit('https://a.test/', 'A');
    library.toggle('https://a.test/', 'A');
    switchTo('second');
    assert.deepEqual(library.snapshot(), { history: [], bookmarks: [] });
    library.visit('https://b.test/', 'B');
    switchTo('first');
    assert.equal(library.snapshot().history[0].title, 'A');
    assert.equal(library.snapshot().bookmarks[0].title, 'A');
  });
  it('never saves internal URLs, executable URLs, or embedded credentials', () => {
    const { library } = setup();
    for (const url of [
      'oya:home',
      'about:blank',
      'javascript:alert(1)',
      'file:///private',
      'https://user:password@a.test/',
    ]) {
      library.visit(url, 'Private');
      library.toggle(url, 'Private');
    }
    assert.deepEqual(library.snapshot(), { history: [], bookmarks: [] });
  });
  it('toggling a bookmark removes it without clearing history', () => {
    const { library } = setup();
    library.visit('https://a.test/', 'A');
    library.toggle('https://a.test/', 'A');
    library.toggle('https://a.test/', 'A');
    assert.equal(library.snapshot().bookmarks.length, 0);
    assert.equal(library.snapshot().history.length, 1);
  });
  it('clearing history keeps bookmarks and other profiles intact', () => {
    const { library, switchTo } = setup();
    library.visit('https://a.test/', 'A');
    switchTo('second');
    library.visit('https://b.test/', 'B');
    library.toggle('https://b.test/', 'B');
    library.clearHistory();
    assert.equal(library.snapshot().history.length, 0);
    assert.equal(library.snapshot().bookmarks.length, 1);
    switchTo('first');
    assert.equal(library.snapshot().history.length, 1);
  });
  it('bounds recent history and ignores corrupt persisted entries', () => {
    const { ctx, library } = setup();
    ctx.config.values.browsingLibrary = { first: { history: [null, { url: 'javascript:alert(1)' }], bookmarks: 42 } };
    assert.deepEqual(library.snapshot(), { history: [], bookmarks: [] });
    for (let index = 0; index <= HISTORY_LIMIT; index++) library.visit(`https://a.test/${index}`, 'A');
    assert.equal(library.snapshot().history.length, HISTORY_LIMIT);
  });
});
