/** Native library commands are scoped to human control and the current persona. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BrowsingLibrary, LibraryMenu } from '../../../../src/main/library/index.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
import { incognitoSession } from '../../support/incognito.ts';

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
  const item = (start: string) => items.find((entry) => entry.label?.startsWith(start));
  return { ctx, opened, menu: new LibraryMenu(ctx), items: () => items, item };
}
describe('LibraryMenu', () => {
  it('keeps history maintenance out of the bookmarks menu', () => {
    const { menu, ctx, items } = setup();
    menu.toggle();
    assert.equal(ctx.library.snapshot().bookmarks.length, 1);
    menu.manage('bookmarks');
    assert.equal(
      items().some((item) => item.label?.includes('history')),
      false,
    );
    menu.toggle();
    assert.deepEqual(ctx.library.snapshot().bookmarks, []);
  });
  it('saves nothing from an incognito page', async () => {
    const { menu, ctx } = setup();
    const session = await incognitoSession();
    ctx.tabs.find = () => ({ id: 1, url: 'https://a.test/', title: 'A', view: { webContents: { session } } });
    menu.toggle();
    assert.deepEqual(ctx.library.snapshot().bookmarks, []);
  });
  it('refuses bookmarking while an agent owns the page', () => {
    const { menu, ctx } = setup();
    ctx.control.snapshot = () => ({ interactive: false });
    menu.toggle();
    assert.deepEqual(ctx.library.snapshot().bookmarks, []);
  });
  it('requires confirmation before clearing history', async () => {
    const { menu, ctx, item } = setup();
    ctx.library.visit('https://a.test/', 'A');
    menu.manage();
    ctx.electron.dialog.showMessageBox = async () => ({ response: 0 });
    item('Clear browsing history').click();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(ctx.library.snapshot().history.length, 1);
    ctx.electron.dialog.showMessageBox = async () => ({ response: 1 });
    item('Clear browsing history').click();
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(ctx.library.snapshot().history.length, 0);
  });
  it('does not clear another persona when a confirmation completes late', async () => {
    const { menu, ctx, item } = setup();
    ctx.library.visit('https://a.test/', 'A');
    menu.manage();
    let answer;
    ctx.electron.dialog.showMessageBox = () =>
      new Promise((resolve) => {
        answer = resolve;
      });
    item('Clear browsing history').click();
    ctx.persona.partitionName = () => 'second';
    ctx.library.visit('https://b.test/', 'B');
    answer({ response: 1 });
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(ctx.library.snapshot().history.length, 1);
  });
  it('offers Forget site permissions with the count, disabled when none, and forgets without a confirmation', () => {
    const { menu, ctx, item } = setup();
    menu.manage();
    assert.equal(item('Forget site permissions').label, 'Forget site permissions');
    assert.equal(item('Forget site permissions').enabled, false);
    ctx.sitePermissions.grant(ctx.persona.partitionName(), 'https://call.test', ['audio']);
    ctx.sitePermissions.grant(ctx.persona.partitionName(), 'https://meet.test', ['screen']);
    menu.manage();
    assert.equal(item('Forget site permissions').label, 'Forget site permissions (2 sites)');
    item('Forget site permissions').click();
    assert.deepEqual(ctx.sitePermissions.origins(ctx.persona.partitionName()), []);
  });
  it('does not forget another profile switched to while the menu was open', () => {
    const { menu, ctx, item } = setup();
    const first = ctx.persona.partitionName();
    ctx.sitePermissions.grant(first, 'https://call.test', ['audio']);
    menu.manage();
    ctx.persona.partitionName = () => 'second';
    item('Forget site permissions').click();
    assert.deepEqual(ctx.sitePermissions.origins(first), ['https://call.test']);
  });
});
