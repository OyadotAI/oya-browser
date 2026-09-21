/**
 * Windows the page opened, kept on the tab list.
 *
 * A sign-on handoff, a payer passing a case to a delegated vendor, calls
 * `window.open` and then keeps using what it got back, so those windows have to
 * stay real windows rather than become tabs. That left them somewhere an agent
 * could neither see nor drive, so they are adopted here: listed, switched to and
 * closed like any tab, while remaining their own window on screen.
 */

/** A window the page opened, as the tab list sees it: only the webContents a command needs. */
function windowTab(id, win) {
  return {
    id,
    window: win,
    view: { webContents: win.webContents },
    title: win.webContents.getTitle?.() || 'New Tab',
    url: win.webContents.getURL?.() || '',
  };
}

/** Puts `win` on the manager's list and returns its tab id, or null if it is already there. */
function adoptWindow(manager, win) {
  if (!win?.webContents || manager.list.some((t) => t.window === win)) return null;
  const tab = windowTab(manager.nextTabId++, win);
  manager.list.push(tab);
  win.webContents.on('page-title-updated', (_e, title) => manager.titleChanged(tab, title));
  win.on('closed', () => forgetWindow(manager, win));
  manager.activeTabId = tab.id;
  manager.sendTabList();
  return tab.id;
}

/** Drops a popup's tab when the window itself goes. */
function forgetWindow(manager, win) {
  const idx = manager.list.findIndex((t) => t.window === win);
  if (idx === -1) return;
  if (manager.list[idx].id === manager.activeTabId) manager.activeTabId = null;
  manager.list.splice(idx, 1);
  manager.activeTabId = manager.activeTabId ?? manager.list[0]?.id ?? null;
  manager.sendTabList();
}

/** Brings a popup's own window to the front; there is nothing to mount on the shell. */
function raiseWindow(manager, tab) {
  try {
    tab.window.focus();
  } catch {}
  manager.sendTabList();
}

/** Closes a popup's window. Its own 'closed' handler has already taken it off the list. */
function closeWindowTab(tab) {
  try {
    if (!tab.window.isDestroyed()) tab.window.close();
  } catch {}
}

module.exports = { windowTab, adoptWindow, forgetWindow, raiseWindow, closeWindowTab };
