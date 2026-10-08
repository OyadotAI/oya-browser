/** Native library commands are scoped to human control and the current persona. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BrowsingLibrary, LibraryMenu } from '../../../../src/main/library/index.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** A menu with recordable native APIs and a real in-memory library. */
function setup() {
  const ctx = mainCtx();
  ctx.library = new BrowsingLibrary(ctx);
  const tab = { id: 1, url: 'https://a.test/', title: 'A' };
  const opened = [];
  ctx.tabs = { activeTabId: 1, find: () => tab, createTab: (url) => opened.push(url) };
  let items;
  ctx.electron.Menu.buildFromTemplate = (template) => {
    items = template;
    return { popup() {} };
  };
  return { ctx, opened, menu: new LibraryMenu(ctx), items: () => items };
}
describe('LibraryMenu', () => {
  it('opens saved pages in a new tab and toggles the active bookmark', () => {
    const { menu, ctx, items, opened } = setup();
    menu.toggle();
    menu.show();
    assert.equal(items()[0].label, 'Remove bookmark for this page');
    items()[1].submenu[0].click();
    assert.deepEqual(opened, ['https://a.test/']);
    items()[0].click();
    assert.deepEqual(ctx.library.snapshot().bookmarks, []);
  });
  it('does not reopen a previous persona’s menu entry after switching profiles', () => {
    const { menu, ctx, items, opened } = setup();
    menu.toggle();
    menu.show();
    ctx.persona.partitionName = () => 'another';
    items()[1].submenu[0].click();
    assert.deepEqual(opened, []);
  });
  it('refuses page actions while an agent has control', () => {
    const { menu, ctx, items, opened } = setup();
    menu.toggle();
    menu.show();
    ctx.control.snapshot = () => ({ interactive: false });
    items()[1].submenu[0].click();
    menu.toggle();
    assert.deepEqual(opened, []);
    assert.equal(ctx.library.snapshot().bookmarks.length, 1);
  });
  it('requires confirmation before clearing history', async () => {
    const { menu, ctx, items } = setup();
    ctx.library.visit('https://a.test/', 'A');
    menu.show();
    ctx.electron.dialog.showMessageBox = async () => ({ response: 0 });
    items().at(-1).click();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(ctx.library.snapshot().history.length, 1);
    ctx.electron.dialog.showMessageBox = async () => ({ response: 1 });
    items().at(-1).click();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(ctx.library.snapshot().history.length, 0);
  });
  it('does not clear another persona when a confirmation completes late', async () => {
    const { menu, ctx, items } = setup();
    ctx.library.visit('https://a.test/', 'A');
    menu.show();
    let answer;
    ctx.electron.dialog.showMessageBox = () =>
      new Promise((resolve) => {
        answer = resolve;
      });
    items().at(-1).click();
    ctx.persona.partitionName = () => 'second';
    ctx.library.visit('https://b.test/', 'B');
    answer({ response: 1 });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(ctx.library.snapshot().history.length, 1);
  });
});
