/**
 * The application menu. On macOS the first menu is the app's own; the page
 * commands (new tab, close tab, reload) only act while a person has control.
 */
import { bookmarksBarShown, setBookmarksBar } from './bookmarks-bar.ts';
import type { MenuItem, MenuItemConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { DefaultBrowser } from '../app/default-browser.ts';
import { LibraryMenu } from '../library/index.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { ZOOM_STEP } from './constants.ts';
import { zoomShownPage } from './page-zoom.ts';

/** The services the menu uses. */
type Deps = Pick<
  AppServices,
  | 'appDir'
  | 'electron'
  | 'tabs'
  | 'control'
  | 'library'
  | 'downloads'
  | 'persona'
  | 'shell'
  | 'windows'
  | 'agentTab'
  | 'config'
  | 'sitePermissions'
  | 'findBar'
>;

/** A menu item's id, label and keys. */
type ItemName = Pick<MenuItemConstructorOptions, 'id' | 'label' | 'accelerator'>;

/** Quit, named for the product. */
const QUIT: MenuItemConstructorOptions = { role: 'quit', label: 'Quit Oya Browser' };
/** macOS's app menu below About and Make Default, named for the product rather than Electron. */
const MAC_APP_ITEMS: MenuItemConstructorOptions[] = [
  { type: 'separator' },
  { role: 'services' },
  { type: 'separator' },
  { role: 'hide', label: 'Hide Oya Browser' },
  { role: 'hideOthers' },
  { role: 'unhide' },
  { type: 'separator' },
  QUIT,
];

/** macOS's app menu, named for the product rather than Electron. */
function macAppMenu(deps: Deps): MenuItemConstructorOptions {
  const about: MenuItemConstructorOptions = { role: 'about', label: 'About Oya Browser' };
  return { label: 'Oya Browser', submenu: [about, makeDefaultItem(deps), ...MAC_APP_ITEMS] };
}

/** Where people look for it first, as in Safari and Chrome; Help keeps a route on every platform. */
function makeDefaultItem(deps: Deps): MenuItemConstructorOptions {
  return { label: 'Make Oya Browser Default…', click: () => void new DefaultBrowser(deps).request() };
}

/**
 * A zoom item that zooms the page, never the shell, and only while the person may
 * use it. The built-in zoom roles act on whatever has focus, and while an agent
 * drives that is the shell: its CSS shrinks while the page's view stays placed in
 * window pixels, leaving gaps.
 */
function zoomItem(deps: Deps, item: ItemName, level: (current: number) => number): MenuItemConstructorOptions {
  return { ...item, click: () => zoomShownPage(deps, level) };
}

/** View's zoom and full-screen items, after Reload. */
function viewItems(deps: Deps): MenuItemConstructorOptions[] {
  return [
    { type: 'separator' },
    zoomItem(deps, { id: 'zoom-reset', label: 'Actual Size', accelerator: 'CmdOrCtrl+0' }, () => 0),
    zoomItem(deps, { id: 'zoom-in', label: 'Zoom In', accelerator: 'CmdOrCtrl+=' }, (level) => level + ZOOM_STEP),
    zoomItem(deps, { id: 'zoom-out', label: 'Zoom Out', accelerator: 'CmdOrCtrl+-' }, (level) => level - ZOOM_STEP),
    ...bookmarksBarItems(deps),
    { role: 'togglefullscreen' },
  ];
}

/** Always Show Bookmarks Bar, as in Chrome: Cmd/Ctrl+Shift+B shows or hides it. */
const BOOKMARKS_BAR_ITEM = {
  id: 'bookmarks-bar',
  label: 'Always Show Bookmarks Bar',
  type: 'checkbox',
  accelerator: 'CmdOrCtrl+Shift+B',
} as const;

/** The bookmarks bar's View item, between separators; its tick is whether the bar shows. */
function bookmarksBarItems(deps: Deps): MenuItemConstructorOptions[] {
  const checked = bookmarksBarShown(deps.config);
  const click = (item: MenuItem) => setBookmarksBar(deps as AppServices, item.checked);
  return [{ type: 'separator' }, { ...BOOKMARKS_BAR_ITEM, checked, click }, { type: 'separator' }];
}

/** A page command: it does nothing unless the person may use the tab shown (under the agent, any but its own). */
function humanItem(
  deps: Deps,
  id: string,
  label: string,
  accelerator: string,
  run: () => unknown,
): MenuItemConstructorOptions {
  return { id, label, accelerator, click: () => deps.agentTab.humanMay(deps.tabs.activeTabId) && run() };
}

/** File: tabs, plus Quit where the platform puts it there. */
function fileMenu(deps: Deps, mac: boolean): MenuItemConstructorOptions {
  const newTab = humanItem(deps, 'browser-new-tab', 'New Tab', 'CmdOrCtrl+T', () =>
    deps.tabs.createTab(HOME_URL, true),
  );
  const close = (): unknown => deps.tabs.closeTab(deps.tabs.activeTabId!);
  const closeTab = humanItem(deps, 'browser-close-tab', 'Close Tab', 'CmdOrCtrl+W', close);
  const newWindow = humanItem(deps, 'browser-new-window', 'New Window', 'CmdOrCtrl+N', () => deps.windows?.newWindow());
  return { label: 'File', submenu: [newWindow, newTab, closeTab, ...(mac ? [] : [QUIT])] };
}

/** The standard editing items, spelled out so Find can follow them (the built-in editMenu role takes no extra items). */
const EDIT_ITEMS: MenuItemConstructorOptions[] = [
  { role: 'undo' },
  { role: 'redo' },
  { type: 'separator' },
  { role: 'cut' },
  { role: 'copy' },
  { role: 'paste' },
  { role: 'pasteAndMatchStyle' },
  { role: 'delete' },
  { role: 'selectAll' },
  { type: 'separator' },
];

/** Edit: the standard items, then Find in page with the keys Safari and Chrome use. */
function editMenu(deps: Deps): MenuItemConstructorOptions {
  const find = humanItem(deps, 'browser-find', 'Find…', 'CmdOrCtrl+F', () => deps.findBar.open());
  const next = humanItem(deps, 'browser-find-next', 'Find Next', 'CmdOrCtrl+G', () => deps.findBar.step(true));
  const back = () => deps.findBar.step(false);
  const previous = humanItem(deps, 'browser-find-previous', 'Find Previous', 'Shift+CmdOrCtrl+G', back);
  return { label: 'Edit', submenu: [...EDIT_ITEMS, find, next, previous] };
}

/** View: reload, zoom and full screen. */
function viewMenu(deps: Deps): MenuItemConstructorOptions {
  const reload = humanItem(deps, 'browser-reload', 'Reload Page', 'CmdOrCtrl+R', () => deps.tabs.reloadActivePage());
  return { label: 'View', submenu: [reload, ...viewItems(deps)] };
}

/** Library is reachable from the native menu and keyboard, not just the toolbar. */
function libraryMenus(deps: Deps): MenuItemConstructorOptions[] {
  const menu = new LibraryMenu(deps);
  const history = humanItem(deps, 'browser-library', 'Show History', 'CmdOrCtrl+Y', () => menu.show('history'));
  const bookmarks = bookmarksItem(deps, menu);
  const save = humanItem(deps, 'browser-bookmark', 'Toggle Bookmark', 'CmdOrCtrl+D', () => menu.toggle());
  return [
    { label: 'Bookmarks', submenu: [bookmarks, save] },
    { label: 'History', submenu: [history, downloadsItem(deps, menu)] },
  ];
}

/** The Downloads page, on Chrome's shortcut: Cmd+Shift+J on a Mac, Ctrl+J elsewhere. */
function downloadsItem(deps: Deps, menu: LibraryMenu): MenuItemConstructorOptions {
  const key = process.platform === 'darwin' ? 'Cmd+Shift+J' : 'Ctrl+J';
  return humanItem(deps, 'browser-downloads', 'Show Downloads', key, () => menu.show('downloads'));
}

/** The bookmark manager has its own native shortcut and menu. */
function bookmarksItem(deps: Deps, menu: LibraryMenu): MenuItemConstructorOptions {
  return humanItem(deps, 'browser-bookmarks', 'Show Bookmarks', 'CmdOrCtrl+Shift+O', () => menu.show('bookmarks'));
}

/** Builds and installs the menu. */
export function installApplicationMenu(deps: Deps): void {
  const { app, Menu } = deps.electron;
  const mac = process.platform === 'darwin';
  app.setAboutPanelOptions({ applicationName: 'Oya Browser', applicationVersion: app.getVersion() });
  const template = [...(mac ? [macAppMenu(deps)] : []), fileMenu(deps, mac), editMenu(deps)];
  const trailing: MenuItemConstructorOptions[] = [viewMenu(deps), { role: 'windowMenu' }, defaultMenu(deps)];
  Menu.setApplicationMenu(Menu.buildFromTemplate([...template, ...libraryMenus(deps), ...trailing]));
}

/** A persistent route after the welcome screen is dismissed. */
function defaultMenu(deps: Deps): MenuItemConstructorOptions {
  return {
    label: 'Help',
    submenu: [makeDefaultItem(deps)],
  };
}
