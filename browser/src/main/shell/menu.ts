/**
 * The application menu. On macOS the first menu is the app's own; the page
 * commands (new tab, close tab, reload) only act while a person has control.
 */
import type { MenuItemConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { DefaultBrowser } from '../app/default-browser.ts';
import { LibraryMenu } from '../library/index.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { ZOOM_STEP } from './constants.ts';

/** The services the menu uses. */
type Deps = Pick<AppServices, 'electron' | 'tabs' | 'control' | 'library' | 'persona' | 'shell'>;

/** A menu item's id, label and keys. */
type ItemName = Pick<MenuItemConstructorOptions, 'id' | 'label' | 'accelerator'>;

/** Quit, named for the product. */
const QUIT: MenuItemConstructorOptions = { role: 'quit', label: 'Quit Oya Browser' };
/** macOS's app menu, named for the product rather than Electron. */
const MAC_APP_MENU: MenuItemConstructorOptions = {
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
function zoomItem(deps: Deps, item: ItemName, level: (current: number) => number): MenuItemConstructorOptions {
  const click = (): void => {
    const contents = deps.tabs.getActiveView()?.webContents;
    if (contents) contents.setZoomLevel(level(contents.getZoomLevel()));
  };
  return { ...item, click };
}

/** View's zoom and full-screen items, after Reload. */
function viewItems(deps: Deps): MenuItemConstructorOptions[] {
  return [
    { type: 'separator' },
    zoomItem(deps, { id: 'zoom-reset', label: 'Actual Size', accelerator: 'CmdOrCtrl+0' }, () => 0),
    zoomItem(deps, { id: 'zoom-in', label: 'Zoom In', accelerator: 'CmdOrCtrl+=' }, (level) => level + ZOOM_STEP),
    zoomItem(deps, { id: 'zoom-out', label: 'Zoom Out', accelerator: 'CmdOrCtrl+-' }, (level) => level - ZOOM_STEP),
    { type: 'separator' },
    { role: 'togglefullscreen' },
  ];
}

/** A page command: it does nothing unless a person has control. */
function humanItem(
  deps: Deps,
  id: string,
  label: string,
  accelerator: string,
  run: () => unknown,
): MenuItemConstructorOptions {
  return { id, label, accelerator, click: () => deps.control.snapshot().interactive && run() };
}

/** File: tabs, plus Quit where the platform puts it there. */
function fileMenu(deps: Deps, mac: boolean): MenuItemConstructorOptions {
  const newTab = humanItem(deps, 'browser-new-tab', 'New Tab', 'CmdOrCtrl+T', () =>
    deps.tabs.createTab(HOME_URL, true),
  );
  const close = (): unknown => deps.tabs.closeTab(deps.tabs.activeTabId!);
  const closeTab = humanItem(deps, 'browser-close-tab', 'Close Tab', 'CmdOrCtrl+W', close);
  return { label: 'File', submenu: [newTab, closeTab, ...(mac ? [] : [QUIT])] };
}

/** View: reload, zoom and full screen. */
function viewMenu(deps: Deps): MenuItemConstructorOptions {
  const reload = humanItem(deps, 'browser-reload', 'Reload Page', 'CmdOrCtrl+R', () => deps.tabs.reloadActivePage());
  return { label: 'View', submenu: [reload, ...viewItems(deps)] };
}

/** Library is reachable from the native menu and keyboard, not just the toolbar. */
function libraryMenu(deps: Deps): MenuItemConstructorOptions {
  const menu = new LibraryMenu(deps);
  return {
    label: 'Library',
    submenu: [
      humanItem(deps, 'browser-library', 'History and Bookmarks', 'CmdOrCtrl+Y', () => menu.show()),
      humanItem(deps, 'browser-bookmark', 'Toggle Bookmark', 'CmdOrCtrl+D', () => menu.toggle()),
    ],
  };
}

/** Builds and installs the menu. */
export function installApplicationMenu(deps: Deps): void {
  const { app, Menu } = deps.electron;
  const mac = process.platform === 'darwin';
  app.setAboutPanelOptions({ applicationName: 'Oya Browser', applicationVersion: app.getVersion() });
  const template = [...(mac ? [MAC_APP_MENU] : []), fileMenu(deps, mac), { role: 'editMenu' as const }];
  Menu.setApplicationMenu(
    Menu.buildFromTemplate([...template, libraryMenu(deps), viewMenu(deps), { role: 'windowMenu' }, defaultMenu(deps)]),
  );
}

/** A persistent route after the welcome screen is dismissed. */
function defaultMenu(deps: Deps): MenuItemConstructorOptions {
  return {
    label: 'Help',
    submenu: [{ label: 'Make Oya Browser Default…', click: () => void new DefaultBrowser(deps).request() }],
  };
}
