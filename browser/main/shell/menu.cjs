/**
 * The application menu. On macOS the first menu is the app's own; the page
 * commands (new tab, close tab, reload) only act while a person has control.
 */
const { HOME_URL } = require('../tabs/constants.cjs');
const { ZOOM_STEP } = require('./constants.cjs');

/** Quit, named for the product. */
const QUIT = { role: 'quit', label: 'Quit Oya Browser' };
/** macOS's app menu, named for the product rather than Electron. */
const MAC_APP_MENU = {
  label: 'Oya Browser',
  submenu: [
    { role: 'about', label: 'About Oya Browser' },
    { type: 'separator' },
    { role: 'services' },
    { type: 'separator' },
    { role: 'hide', label: 'Hide Oya Browser' },
    { role: 'hideOthers' },
    { role: 'unhide' },
    { type: 'separator' },
    QUIT,
  ],
};
/**
 * A zoom item that zooms the page, never the shell. The built-in zoom roles act
 * on whatever has focus, and while an agent drives that is the shell: its CSS
 * shrinks while the page's view stays placed in window pixels, leaving gaps.
 */
function zoomItem(ctx, id, label, accelerator, level) {
  const click = () => {
    const contents = ctx.tabs.getActiveView()?.webContents;
    if (contents) contents.setZoomLevel(level(contents.getZoomLevel()));
  };
  return { id, label, accelerator, click };
}

/** View's zoom and full-screen items, after Reload. */
function viewItems(ctx) {
  return [
    { type: 'separator' },
    zoomItem(ctx, 'zoom-reset', 'Actual Size', 'CmdOrCtrl+0', () => 0),
    zoomItem(ctx, 'zoom-in', 'Zoom In', 'CmdOrCtrl+=', (level) => level + ZOOM_STEP),
    zoomItem(ctx, 'zoom-out', 'Zoom Out', 'CmdOrCtrl+-', (level) => level - ZOOM_STEP),
    { type: 'separator' },
    { role: 'togglefullscreen' },
  ];
}

/** A page command: it does nothing unless a person has control. */
function humanItem(ctx, id, label, accelerator, run) {
  return { id, label, accelerator, click: () => ctx.control.snapshot().interactive && run() };
}

/** File: tabs, plus Quit where the platform puts it there. */
function fileMenu(ctx, mac) {
  const newTab = humanItem(ctx, 'browser-new-tab', 'New Tab', 'CmdOrCtrl+T', () => ctx.tabs.createTab(HOME_URL, true));
  const close = () => ctx.tabs.closeTab(ctx.tabs.activeTabId);
  const closeTab = humanItem(ctx, 'browser-close-tab', 'Close Tab', 'CmdOrCtrl+W', close);
  return { label: 'File', submenu: [newTab, closeTab, ...(mac ? [] : [QUIT])] };
}

/** View: reload, zoom and full screen. */
function viewMenu(ctx) {
  const reload = humanItem(ctx, 'browser-reload', 'Reload Page', 'CmdOrCtrl+R', () => ctx.tabs.reloadActivePage());
  return { label: 'View', submenu: [reload, ...viewItems(ctx)] };
}

/** Builds and installs the menu. */
function installApplicationMenu(ctx) {
  const { app, Menu } = ctx.electron;
  const mac = process.platform === 'darwin';
  app.setAboutPanelOptions({ applicationName: 'Oya Browser', applicationVersion: app.getVersion() });
  const template = [...(mac ? [MAC_APP_MENU] : []), fileMenu(ctx, mac), { role: 'editMenu' }];
  Menu.setApplicationMenu(Menu.buildFromTemplate([...template, viewMenu(ctx), { role: 'windowMenu' }]));
}

module.exports = { installApplicationMenu };
