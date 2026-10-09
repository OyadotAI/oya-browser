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
      'Move Tab to New Window',
      'Close Tab',
      'Close Other Tabs',
      'Close Tabs to the Right',
      'Reopen Closed Tab',
    ]);
    assert.equal(items['Close Tabs to the Right'].enabled, false, 'the last tab has nothing to its right');
    assert.equal(items['Reopen Closed Tab'].enabled, false, 'nothing was closed yet');
  });

  it('duplicates a tab beside it and opens a new tab to its right', () => {
    menuFor(1).Duplicate.click();
    assert.equal(ctx.tabs.list[1].url, 'https://a.test/');
    menuFor(1)['New Tab to the Right'].click();
    assert.ok(ctx.tabs.list[1].home);
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
