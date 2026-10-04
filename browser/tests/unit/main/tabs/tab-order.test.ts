/**
 * Unit tests for the strip's order: moving a tab (the main process's order is
 * what the strip shows), opening beside, closing others or to the right, and
 * reopening closed tabs where they were.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import * as order from '../../../../src/main/tabs/tab-order.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

const { ClosedTabs, reorder, moveTabTo, openBeside, closeOthers, closeToRight, reopenClosed } = order;

describe('reorder', () => {
  it('moves one item and keeps the rest in order, clamping the target', () => {
    assert.deepEqual(reorder(['a', 'b', 'c', 'd'], 0, 2), ['b', 'c', 'a', 'd']);
    assert.deepEqual(reorder(['a', 'b', 'c', 'd'], 3, 0), ['d', 'a', 'b', 'c']);
    assert.deepEqual(reorder(['a', 'b', 'c'], 0, 99), ['b', 'c', 'a']);
    assert.deepEqual(reorder(['a', 'b', 'c'], 2, -5), ['c', 'a', 'b']);
  });
});

describe('ClosedTabs', () => {
  it('keeps the newest 25 and hands back the newest first', () => {
    const closed = new ClosedTabs();
    for (let i = 0; i < 30; i++) closed.push(`https://a.test/${i}`, i);
    assert.equal(closed.size, 25);
    assert.deepEqual(closed.pop(), { url: 'https://a.test/29', index: 29 });
    assert.equal(closed.entries[0].url, 'https://a.test/5', 'the oldest five were dropped');
  });

  it('does not remember a blank tab', () => {
    const closed = new ClosedTabs();
    closed.push('', 0);
    closed.push('about:blank', 0);
    assert.equal(closed.size, 0);
    assert.equal(closed.pop(), undefined);
  });
});

describe('the strip order', () => {
  let ctx;
  /** The ids on the strip, in order. */
  const ids = () => ctx.tabs.list.map((t) => t.id);
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    for (const site of ['a', 'b', 'c', 'd']) ctx.tabs.createTab(`https://${site}.test/`);
  });
  afterEach(() => mock.timers.reset());

  it('moves a tab and tells the strip the new order', () => {
    assert.equal(moveTabTo(ctx.tabs, 1, 2), true);
    assert.deepEqual(ids(), [2, 3, 1, 4]);
    assert.deepEqual(
      ctx.shell
        .sentOn('tabs-updated')
        .at(-1)
        .map((t) => t.id),
      [2, 3, 1, 4],
    );
  });

  it('refuses an unknown tab, a non-integer place and a move that changes nothing', () => {
    const sent = ctx.shell.sentOn('tabs-updated').length;
    assert.equal(moveTabTo(ctx.tabs, 99, 0), false);
    assert.equal(moveTabTo(ctx.tabs, 1, '2'), false);
    assert.equal(moveTabTo(ctx.tabs, 1, 0), false);
    assert.equal(ctx.shell.sentOn('tabs-updated').length, sent);
  });

  it('opens a tab just right of another and shows it', () => {
    const id = openBeside(ctx.tabs, 2, 'https://e.test/');
    assert.deepEqual(ids(), [1, 2, id, 3, 4]);
    assert.equal(ctx.tabs.activeTabId, id);
  });

  it('closes every other tab and shows the one kept', () => {
    closeOthers(ctx.tabs, 2);
    assert.deepEqual(ids(), [2]);
    assert.equal(ctx.tabs.activeTabId, 2);
  });

  it('closes only the tabs right of one', () => {
    closeToRight(ctx.tabs, 2);
    assert.deepEqual(ids(), [1, 2]);
    closeToRight(ctx.tabs, 99);
    assert.deepEqual(ids(), [1, 2]);
  });

  it('reopens closed tabs newest first, each where it was', () => {
    ctx.tabs.closeTab(2);
    ctx.tabs.closeTab(4);
    const d = reopenClosed(ctx.tabs);
    assert.equal(ctx.tabs.find(d).url, 'https://d.test/');
    assert.equal(ids().indexOf(d), 2, 'd was third when it closed');
    const b = reopenClosed(ctx.tabs);
    assert.equal(ctx.tabs.find(b).url, 'https://b.test/');
    assert.equal(ids().indexOf(b), 1);
    assert.equal(ctx.tabs.activeTabId, b);
    assert.equal(reopenClosed(ctx.tabs), undefined, 'nothing is left to reopen');
  });

  it('never remembers the start page, and forgets everything on a bulk close', () => {
    const home = ctx.tabs.createTab('oya:home');
    ctx.tabs.closeTab(home);
    assert.equal(ctx.tabs.closed.size, 0);
    ctx.tabs.closeTab(1);
    assert.equal(ctx.tabs.closed.size, 1);
    ctx.tabs.closeTab(2, { keepOne: false });
    assert.equal(ctx.tabs.closed.size, 0, 'a log out or profile switch must not reopen the last profile');
  });
});
