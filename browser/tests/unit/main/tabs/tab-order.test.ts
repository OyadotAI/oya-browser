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
import { incognitoSession } from '../../support/incognito.ts';

const { ClosedTabs, reorder, moveTabTo, openBeside, closeOthers, closeToRight, reopenClosed, reopenable } = order;
const { pinnable, pinnedCount, setPinned, openLink } = order;

describe('reopenable', () => {
  it('excludes the start page, popups, blank tabs and tabs with no address', () => {
    const page = { url: 'https://a.test/', view: { webContents: { session: {} } } };
    assert.equal(reopenable(page as any), true);
    assert.equal(reopenable({ ...page, home: true } as any), false);
    assert.equal(reopenable({ ...page, window: {} } as any), false);
    assert.equal(reopenable({ ...page, url: 'about:blank' } as any), false);
    assert.equal(reopenable({ ...page, url: '' } as any), false);
    assert.equal(reopenable({ url: 'https://a.test/', view: {} } as any), true, 'a tab with no session yet');
  });

  it('reopens an incognito tab only in its own session, under the same rules as any tab', async () => {
    const session = await incognitoSession();
    const tab = { url: 'https://a.test/', view: { webContents: { session } } };
    assert.equal(reopenable(tab as any), false, 'private to every other window');
    assert.equal(reopenable(tab as any, session), true);
    assert.equal(reopenable({ ...tab, home: true } as any, session), false, 'never a start page');
  });
});

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

  describe('opening a link in a new tab', () => {
    /** Tab `id`'s page, as the link's opener. */
    const page = (id: number) => ctx.tabs.find(id).view;
    beforeEach(() => ctx.tabs.activateTab(2));

    it('opens it in the background, so the person stays on the page', () => {
      const id = openLink(ctx.tabs, { page: page(2), url: 'https://e.test/' });
      assert.equal(ctx.tabs.activeTabId, 2);
      assert.equal(ctx.tabs.find(id).openerId, 2);
    });

    it('puts it right of the page it was on', () => {
      const id = openLink(ctx.tabs, { page: page(2), url: 'https://e.test/' });
      assert.deepEqual(ids(), [1, 2, id, 3, 4]);
    });

    it('keeps links opened in a row in the order they were opened', () => {
      const first = openLink(ctx.tabs, { page: page(2), url: 'https://e.test/' });
      const second = openLink(ctx.tabs, { page: page(2), url: 'https://f.test/' });
      assert.deepEqual(ids(), [1, 2, first, second, 3, 4]);
    });

    it('keeps the order of links opened in a row from a pinned tab, after the pinned block', () => {
      setPinned(ctx.tabs, 1, true);
      setPinned(ctx.tabs, 2, true);
      const first = openLink(ctx.tabs, { page: page(1), url: 'https://e.test/' });
      const second = openLink(ctx.tabs, { page: page(1), url: 'https://f.test/' });
      assert.deepEqual(ids(), [1, 2, first, second, 3, 4]);
    });

    it('shows it when asked (a plain target=_blank click)', () => {
      const id = openLink(ctx.tabs, { page: page(2), url: 'https://e.test/', show: true });
      assert.equal(ctx.tabs.activeTabId, id);
      assert.deepEqual(ids(), [1, 2, id, 3, 4]);
    });

    it("opens in the page's session, so a persona or private context carries over", () => {
      const session = { cookies: {} };
      page(2).webContents.session = session;
      const id = openLink(ctx.tabs, { page: page(2), url: 'https://e.test/' });
      assert.equal(ctx.tabs.find(id).view.options.webPreferences.session, session);
    });

    it('still opens, at the end, when the page is no longer on the strip', () => {
      const gone = page(2);
      ctx.tabs.closeTab(2);
      const id = openLink(ctx.tabs, { page: gone, url: 'https://e.test/' });
      assert.equal(ids().at(-1), id);
    });
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

describe('pinnable', () => {
  it('refuses the start page, collections, popups and private tabs, which the session file never keeps', () => {
    const page = { url: 'https://a.test/', view: { webContents: { session: {} } } };
    assert.equal(pinnable(page as any), true);
    assert.equal(pinnable({ ...page, home: true } as any), false);
    assert.equal(pinnable({ ...page, collection: 'bookmarks' } as any), false);
    assert.equal(pinnable({ ...page, window: {} } as any), false);
    assert.equal(
      pinnable({ ...page, url: 'file:///Users/a/x.pdf' } as any),
      false,
      'never restored, so the pin would be lost',
    );
  });
});

describe('pinned tabs on the strip', () => {
  let ctx;
  /** The ids on the strip, in order. */
  const ids = () => ctx.tabs.list.map((t) => t.id);
  /** The ids that are pinned, in order. */
  const pins = () => ctx.tabs.list.filter((t) => t.pinned).map((t) => t.id);
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    for (const site of ['a', 'b', 'c', 'd']) ctx.tabs.createTab(`https://${site}.test/`);
  });
  afterEach(() => mock.timers.reset());

  it('pinning puts a tab at the end of the pinned block', () => {
    setPinned(ctx.tabs, 3, true);
    setPinned(ctx.tabs, 4, true);
    assert.deepEqual(ids(), [3, 4, 1, 2]);
    assert.equal(pinnedCount(ctx.tabs.list), 2);
  });

  it('unpinning puts a tab at the start of the unpinned block', () => {
    setPinned(ctx.tabs, 3, true);
    setPinned(ctx.tabs, 4, true);
    setPinned(ctx.tabs, 3, false);
    assert.deepEqual(ids(), [4, 3, 1, 2]);
    assert.deepEqual(pins(), [4]);
  });

  it('sends the strip even when the pinned tab did not move, so it redraws and is saved', () => {
    const sent = ctx.shell.sentOn('tabs-updated').length;
    assert.equal(setPinned(ctx.tabs, 1, true), true);
    assert.equal(ctx.shell.sentOn('tabs-updated').length, sent + 1);
    assert.equal(ctx.shell.sentOn('tabs-updated').at(-1)[0].pinned, true);
  });

  it('refuses an unknown tab and a pin that changes nothing', () => {
    assert.equal(setPinned(ctx.tabs, 99, true), false);
    assert.equal(setPinned(ctx.tabs, 1, false), false);
  });

  it('keeps a pinned tab inside the pinned block when moved', () => {
    setPinned(ctx.tabs, 1, true);
    setPinned(ctx.tabs, 2, true);
    moveTabTo(ctx.tabs, 1, 3);
    assert.deepEqual(ids(), [2, 1, 3, 4]);
  });

  it('keeps an unpinned tab out of the pinned block when moved', () => {
    setPinned(ctx.tabs, 1, true);
    moveTabTo(ctx.tabs, 4, 0);
    assert.deepEqual(ids(), [1, 4, 2, 3]);
  });

  it('opens a new tab from a pinned one unpinned, at the first unpinned place', () => {
    setPinned(ctx.tabs, 1, true);
    setPinned(ctx.tabs, 2, true);
    const id = openBeside(ctx.tabs, 1, 'https://e.test/');
    assert.deepEqual(ids(), [1, 2, id, 3, 4]);
    assert.deepEqual(pins(), [1, 2]);
  });

  it('reopens a closed tab unpinned, clamped out of the pinned block', () => {
    ctx.tabs.closeTab(1);
    setPinned(ctx.tabs, 3, true);
    setPinned(ctx.tabs, 4, true);
    const id = reopenClosed(ctx.tabs);
    assert.deepEqual(ids(), [3, 4, id, 2]);
    assert.equal(ctx.tabs.find(id).pinned, undefined);
  });

  it('closing the others spares pinned tabs', () => {
    setPinned(ctx.tabs, 1, true);
    closeOthers(ctx.tabs, 3);
    assert.deepEqual(ids(), [1, 3]);
  });

  it('closing the others from a pinned tab closes every unpinned tab', () => {
    setPinned(ctx.tabs, 1, true);
    setPinned(ctx.tabs, 2, true);
    closeOthers(ctx.tabs, 1);
    assert.deepEqual(ids(), [1, 2]);
  });

  it('closing to the right spares pinned tabs', () => {
    setPinned(ctx.tabs, 1, true);
    setPinned(ctx.tabs, 2, true);
    closeToRight(ctx.tabs, 1);
    assert.deepEqual(ids(), [1, 2]);
  });
});
