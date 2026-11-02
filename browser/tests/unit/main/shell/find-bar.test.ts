/**
 * Unit tests for FindBar: find in page opens only on a page the person may use,
 * a new query starts a search while next and previous continue it, only the final
 * reply to the latest request sets the count, and a tab switch, a new page or the
 * agent taking the tab closes it and clears its highlights.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { FindBar } from '../../../../src/main/shell/find-bar.ts';
import { FIND_BAR_HEIGHT, FIND_QUERY_MAX_CHARS } from '../../../../src/main/shell/constants.ts';

/** A tab's page: records find calls and answers each with the next request id. */
class FakePage extends EventEmitter {
  /** Each findInPage call, as [text, options]. */
  finds: Array<[string, object]> = [];
  /** Each stopFindInPage action. */
  stops: string[] = [];
  /** How often the page took the keyboard. */
  focused = 0;
  /** Set when the page is gone. */
  destroyed = false;
  /** As Electron's: records the call and answers its request id. */
  findInPage(text: string, options: object): number {
    this.finds.push([text, options]);
    return this.finds.length;
  }
  /** As Electron's. */
  stopFindInPage(action: string): void {
    this.stops.push(action);
  }
  /** As Electron's. */
  isDestroyed(): boolean {
    return this.destroyed;
  }
  /** As Electron's. */
  focus(): void {
    this.focused++;
  }
  /** A found-in-page reply. */
  reply(requestId: number, matches: number, activeMatchOrdinal: number, finalUpdate = true): void {
    this.emit('found-in-page', {}, { requestId, matches, activeMatchOrdinal, finalUpdate });
  }
}

describe('FindBar', () => {
  let page: FakePage, other: FakePage, deps: any, bar: FindBar, humanMay: boolean, layouts: number;
  const sent = (channel: string) => deps.shell.sent.filter(([c]: [string]) => c === channel).map(([, d]: any) => d);
  beforeEach(() => {
    page = new FakePage();
    other = new FakePage();
    humanMay = true;
    layouts = 0;
    const tabs: any[] = [
      { id: 1, home: false, view: { webContents: page } },
      { id: 2, home: false, view: { webContents: other } },
      { id: 3, home: true, collection: 'bookmarks', view: { webContents: new FakePage() } },
      { id: 4, home: false, view: { webContents: new FakePage() }, window: {} },
    ];
    deps = {
      tabs: { activeTabId: 1, find: (id: number) => tabs.find((t) => t.id === id) },
      shell: {
        alive: () => true,
        window: { webContents: { focus() {} } },
        sent: [],
        send: (c: string, d: unknown) => deps.shell.sent.push([c, d]),
      },
      layout: { layoutActiveTab: () => layouts++ },
      agentTab: { humanMay: () => humanMay },
    };
    bar = new FindBar(deps);
  });

  it('opens on the shown page, makes room for itself, and asks the shell to focus the field', () => {
    bar.open();
    assert.equal(bar.height(), FIND_BAR_HEIGHT);
    assert.equal(layouts, 1);
    assert.deepEqual(sent('find-bar'), [{ open: true, active: 0, total: null }]);
    assert.deepEqual(sent('shell-command'), ['find']);
  });

  it('does nothing while the agent holds the shown tab', () => {
    humanMay = false;
    bar.open();
    assert.equal(bar.height(), 0);
    assert.deepEqual(deps.shell.sent, []);
  });

  it('hands Cmd/Ctrl+F to a shell page (the library) without opening over it', () => {
    deps.tabs.activeTabId = 3;
    bar.open();
    assert.equal(bar.height(), 0);
    assert.deepEqual(sent('shell-command'), ['find']);
    assert.deepEqual(sent('find-bar'), []);
  });

  it('does nothing on a popup, which has a window of its own', () => {
    deps.tabs.activeTabId = 4;
    bar.open();
    assert.deepEqual(deps.shell.sent, []);
  });

  it('starts a new search for each new query, and continues it for next and previous', () => {
    bar.open();
    bar.search('cat');
    bar.step(true);
    bar.step(false);
    assert.deepEqual(page.finds, [
      ['cat', { findNext: true }],
      ['cat', { forward: true, findNext: false }],
      ['cat', { forward: false, findNext: false }],
    ]);
  });

  it('clears the highlights and the count for an empty query, without searching', () => {
    bar.open();
    bar.search('');
    bar.step(true);
    assert.deepEqual(page.finds, []);
    assert.deepEqual(page.stops, ['clearSelection']);
    assert.deepEqual(sent('find-bar').at(-1), { open: true, active: 0, total: null });
  });

  it('refuses a query that is not text', () => {
    bar.open();
    bar.search(42);
    assert.deepEqual(page.finds, []);
  });

  it('ends the search, its count and its steps for a query too long to be on a page', () => {
    bar.open();
    bar.search('cat');
    bar.search('x'.repeat(FIND_QUERY_MAX_CHARS + 1));
    bar.step(true);
    page.reply(1, 3, 1);
    assert.equal(page.finds.length, 1);
    assert.deepEqual(page.stops, ['clearSelection']);
    assert.deepEqual(sent('find-bar').at(-1), { open: true, active: 0, total: null });
  });

  it('does not search while shut', () => {
    bar.search('cat');
    bar.step(true);
    assert.deepEqual(page.finds, []);
  });

  it('counts only the final reply to the latest request', () => {
    bar.open();
    bar.search('ca');
    bar.search('cat');
    page.reply(1, 9, 1);
    page.reply(2, 3, 1, false);
    page.reply(2, 3, 1);
    assert.deepEqual(sent('find-bar').slice(1), [{ open: true, active: 1, total: 3 }]);
  });

  it('closes on a new page in its tab, but not on a move within the page', () => {
    bar.open();
    page.emit('did-navigate-in-page');
    assert.equal(bar.height(), FIND_BAR_HEIGHT);
    page.emit('did-navigate');
    assert.equal(bar.height(), 0);
    assert.deepEqual(sent('find-bar').at(-1), { open: false, active: 0, total: null });
  });

  it('closes and clears the old page when another tab is shown', () => {
    bar.open();
    deps.tabs.activeTabId = 2;
    bar.follow(2);
    assert.deepEqual(page.stops, ['clearSelection']);
    assert.equal(bar.height(), 0);
  });

  it('closes once the agent takes its tab', () => {
    bar.open();
    humanMay = false;
    bar.sync();
    assert.deepEqual(page.stops, ['clearSelection']);
    assert.deepEqual(sent('find-bar').at(-1), { open: false, active: 0, total: null });
  });

  it('stops listening to the page when it closes', () => {
    const before = page.eventNames().length;
    bar.open();
    bar.close();
    assert.equal(page.eventNames().length, before);
  });

  it('gives the keyboard back to the page on Esc, but not on any other close', () => {
    bar.open();
    bar.close();
    assert.equal(page.focused, 0);
    bar.open();
    bar.dismiss();
    assert.equal(page.focused, 1);
  });

  it('closes without laying out a window that is being torn down', () => {
    bar.open();
    deps.shell.alive = () => false;
    bar.close();
    assert.equal(layouts, 1);
    assert.equal(bar.height(), 0);
  });

  it('closes without touching a page that is gone', () => {
    bar.open();
    page.destroyed = true;
    page.emit('destroyed');
    assert.deepEqual(page.stops, []);
    assert.equal(bar.height(), 0);
  });
});
