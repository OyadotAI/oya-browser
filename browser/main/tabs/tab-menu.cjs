/**
 * The tab strip's right-click menu, a native menu like Chrome's: new tab to
 * the right, reload, duplicate, close, close others, close to the right, and
 * reopen closed tab. Every item acts only while a person has control.
 */
const { HOME_URL } = require('./constants.cjs');
const { openBeside, closeOthers, closeToRight, reopenClosed } = require('./tab-order.cjs');

/** One item that runs `run` only while a person holds control. */
function item(ctx, label, run, enabled = true) {
  return { label, enabled, click: () => ctx.control.snapshot().interactive && run() };
}

/** The items that open or reload: they act on tab `tab`. */
function openItems(ctx, tab) {
  const tabs = ctx.tabs;
  return [
    item(ctx, 'New Tab to the Right', () => openBeside(tabs, tab.id, HOME_URL)),
    { type: 'separator' },
    item(ctx, 'Reload', () => tabs.reloadTab(tab), !tab.home),
    item(ctx, 'Duplicate', () => openBeside(tabs, tab.id, tab.url || HOME_URL)),
  ];
}

/** The items that close, and Reopen closed tab. */
function closeItems(ctx, tab) {
  const tabs = ctx.tabs;
  return [
    item(ctx, 'Close Tab', () => tabs.closeTab(tab.id)),
    item(ctx, 'Close Other Tabs', () => closeOthers(tabs, tab.id), tabs.list.length > 1),
    item(ctx, 'Close Tabs to the Right', () => closeToRight(tabs, tab.id), tabs.list.at(-1) !== tab),
    { type: 'separator' },
    item(ctx, 'Reopen Closed Tab', () => reopenClosed(tabs), tabs.closed.size > 0),
  ];
}

/** The menu's template for tab `tab`. */
function tabMenuTemplate(ctx, tab) {
  return [...openItems(ctx, tab), { type: 'separator' }, ...closeItems(ctx, tab)];
}

/** Shows the menu for tab `id` at the pointer, over the shell window. */
function showTabMenu(ctx, id) {
  const tab = ctx.tabs.find(id);
  if (!tab) return;
  ctx.electron.Menu.buildFromTemplate(tabMenuTemplate(ctx, tab)).popup({ window: ctx.shell.window });
}

module.exports = { showTabMenu, tabMenuTemplate };
