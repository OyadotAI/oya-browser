/**
 * The application menu. On macOS the first menu is the app's own; the page
 * commands (new tab, close tab, reload) only act while a person has control.
 */
const { HOME_URL } = require('../tabs/constants.cjs');

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
/** View's zoom and full-screen items, after Reload. */
const VIEW_ITEMS = [
  { type: 'separator' },
  { role: 'resetZoom' },
  { role: 'zoomIn' },
  { role: 'zoomOut' },
  { type: 'separator' },
  { role: 'togglefullscreen' },
];

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
  return { label: 'View', submenu: [reload, ...VIEW_ITEMS] };
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
