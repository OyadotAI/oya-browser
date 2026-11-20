/**
 * Unit tests for the tab strip's right-click menu: what it offers for a tab,
 * what each item does, and nothing done while an agent has control.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { TabMenu } from '../../../../src/main/tabs/tab-menu.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

describe('tab menu', () => {
  let ctx;
  /** The menu shown for tab `id`, as label → item. */
  const menuFor = (id) => {
    let built;
    ctx.electron.Menu.buildFromTemplate = (template) => (built = { template, popup() {} });
    new TabMenu(ctx).show(id);
    return Object.fromEntries(built.template.filter((i) => i.label).map((i) => [i.label, i]));
  };
  /** The ids on the strip, in order. */
  const ids = () => ctx.tabs.list.map((t) => t.id);
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    for (const site of ['a', 'b', 'c']) ctx.tabs.createTab(`https://${site}.test/`);
  });
  afterEach(() => mock.timers.reset());

  it("offers Chrome's tab items, and disables those with nothing to act on", () => {
    const items = menuFor(3);
    assert.deepEqual(Object.keys(items), [
      'New Tab to the Right',
      'Reload',
      'Duplicate',
      'Pin Tab',
      'Move Tab to New Window',
      'Close Tab',
      'Close Other Tabs',
      'Close Tabs to the Right',
      'Reopen Closed Tab',
    ]);
    assert.equal(items['Close Tabs to the Right'].enabled, false, 'the last tab has nothing to its right');
    assert.equal(items['Reopen Closed Tab'].enabled, false, 'nothing was closed yet');
  });

  it('duplicates a tab beside it, opens a new tab to its right, and disables Duplicate on collections', () => {
    menuFor(1).Duplicate.click();
    assert.equal(ctx.tabs.list[1].url, 'https://a.test/');
    menuFor(1)['New Tab to the Right'].click();
    assert.ok(ctx.tabs.list[1].home);
    ctx.tabs.find(1).collection = 'bookmarks';
    const collectionMenu = menuFor(1);
    assert.equal(collectionMenu.Duplicate.enabled, false);
    const before = ctx.tabs.list.length;
    collectionMenu.Duplicate.click();
    assert.equal(ctx.tabs.list.length, before);
  });

  it('closes, closes the others, and reopens what was closed', () => {
    menuFor(2)['Close Tab'].click();
    assert.deepEqual(ids(), [1, 3]);
    menuFor(1)['Reopen Closed Tab'].click();
    assert.equal(ctx.tabs.list[1].url, 'https://b.test/');
    menuFor(1)['Close Other Tabs'].click();
    assert.deepEqual(ids(), [1]);
  });

  it('does nothing while an agent has control', () => {
    const items = menuFor(1);
    ctx.control.state.interactive = false;
    items['Close Tab'].click();
    items['Close Other Tabs'].click();
    assert.equal(ctx.tabs.list.length, 3);
  });

  it('shows nothing for a tab that is gone', () => {
    let built = false;
    ctx.electron.Menu.buildFromTemplate = () => (built = true);
    new TabMenu(ctx).show(99);
    assert.equal(built, false);
  });
});

describe('tab menu pinning', () => {
  let ctx;
  /** The menu shown for tab `id`, as label → item. */
  const menuFor = (id) => {
    let built;
    ctx.electron.Menu.buildFromTemplate = (template) => (built = { template, popup() {} });
    new TabMenu(ctx).show(id);
    return Object.fromEntries(built.template.filter((i) => i.label).map((i) => [i.label, i]));
  };
  /** The ids on the strip, in order. */
  const ids = () => ctx.tabs.list.map((t) => t.id);
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ tabs: TabManager });
    for (const site of ['a', 'b', 'c']) ctx.tabs.createTab(`https://${site}.test/`);
  });
  afterEach(() => mock.timers.reset());

  it('pins a tab to the left, and a pinned tab offers Unpin Tab in the same place', () => {
    menuFor(3)['Pin Tab'].click();
    assert.deepEqual(ids(), [3, 1, 2]);
    const items = menuFor(3);
    assert.equal(items['Pin Tab'], undefined);
    items['Unpin Tab'].click();
    assert.equal(ctx.tabs.find(3).pinned, false);
  });

  it('disables Pin Tab on the start page and on a collection, which a restart could not bring back', () => {
    const home = ctx.tabs.createTab('oya:home');
    assert.equal(menuFor(home)['Pin Tab'].enabled, false);
    ctx.tabs.find(1).collection = 'history';
    assert.equal(menuFor(1)['Pin Tab'].enabled, false);
  });

  it('disables Pin Tab while an agent drives the tab, and the click rechecks control', () => {
    const items = menuFor(1);
    ctx.control.state.interactive = false;
    items['Pin Tab'].click();
    assert.equal(ctx.tabs.find(1).pinned, undefined);
    assert.equal(menuFor(1)['Pin Tab'].enabled, false);
  });

  it('pins a tab the agent has not claimed while scoped agents drive their own tabs, but never a claimed one', () => {
    Object.assign(ctx.control.state, { connected: true, mode: 'agent', localClients: 0, interactive: false });
    (ctx.agentTab as any).claims.set('lane', 2);
    const items = menuFor(1);
    assert.equal(items['Pin Tab'].enabled, true);
    items['Pin Tab'].click();
    assert.equal(ctx.tabs.find(1).pinned, true, 'an enabled item does what it says');
    assert.equal(menuFor(2)['Pin Tab'].enabled, false);
  });

  it('turns off the bulk closes when only pinned tabs would be left to close', () => {
    menuFor(1)['Pin Tab'].click();
    menuFor(2)['Pin Tab'].click();
    const items = menuFor(1);
    assert.equal(items['Close Other Tabs'].enabled, true, 'c is still unpinned');
    assert.equal(items['Close Tabs to the Right'].enabled, true);
    menuFor(3)['Pin Tab'].click();
    assert.equal(menuFor(1)['Close Other Tabs'].enabled, false);
    assert.equal(menuFor(2)['Close Tabs to the Right'].enabled, false);
  });

  it('closes the others in agent mode with no agent running: a desktop rests in agent mode', () => {
    Object.assign(ctx.control.state, { connected: true, mode: 'agent', localClients: 0, interactive: false });
    menuFor(1)['Close Other Tabs'].click();
    assert.deepEqual(ids(), [1]);
  });

  it("the bulk closes leave a tab an agent is working in, and its own tab's items do nothing", () => {
    Object.assign(ctx.control.state, { connected: true, mode: 'agent', localClients: 0, interactive: false });
    (ctx.agentTab as any).claims.set('lane', 2);
    menuFor(1)['Close Other Tabs'].click();
    assert.deepEqual(ids(), [1, 2]);
    menuFor(2)['Close Tab'].click();
    assert.deepEqual(ids(), [1, 2]);
  });
});
