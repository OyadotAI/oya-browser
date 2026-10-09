/**
 * The tab strip's right-click menu, a native menu like Chrome's: new tab to
 * the right, reload, duplicate, close, close others, close to the right, and
 * reopen closed tab. Every item acts only while a person has control.
 */
import type { MenuItemConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { HOME_URL } from './constants.ts';
import { openBeside, closeOthers, closeToRight, reopenClosed } from './tab-order.ts';
import type { Tab } from './types.ts';

/** The services the menu uses. */
type Deps = Pick<AppServices, 'tabs' | 'control' | 'electron' | 'shell' | 'windows'>;

/** The strip's right-click menu for one tab. */
export class TabMenu {
  /** The tabs it acts on, who has control, and Electron's menus. */
  private readonly deps: Deps;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Shows the menu for tab `id` at the pointer, over the shell window. */
  show(id: number): void {
    const tab = this.deps.tabs.find(id);
    if (!tab) return;
    this.deps.electron.Menu.buildFromTemplate(this.template(tab)).popup({
      window: this.deps.shell.window ?? undefined,
    });
  }

  /** The menu's template for tab `tab`. */
  template(tab: Tab): MenuItemConstructorOptions[] {
    return [...this.openItems(tab), { type: 'separator' }, ...this.closeItems(tab)];
  }

  /** One item that runs `run` only while a person holds control. */
  private item(label: string, run: () => unknown, enabled = true): MenuItemConstructorOptions {
    return { label, enabled, click: () => this.deps.control.snapshot().interactive && run() };
  }

  /** The items that open or reload: they act on tab `tab`. */
  private openItems(tab: Tab): MenuItemConstructorOptions[] {
    const tabs = this.deps.tabs;
    return [
      this.item('New Tab to the Right', () => openBeside(tabs, tab.id, HOME_URL)),
      { type: 'separator' },
      this.item('Reload', () => tabs.reloadTab(tab), !tab.home),
      this.item('Duplicate', () => openBeside(tabs, tab.id, tab.url || HOME_URL)),
      this.item('Move Tab to New Window', () => this.deps.windows?.detach(tab.id), !!this.deps.windows && !tab.window),
    ];
  }

  /** The items that close, and Reopen closed tab. */
  private closeItems(tab: Tab): MenuItemConstructorOptions[] {
    const tabs = this.deps.tabs;
    return [
      this.item('Close Tab', () => tabs.closeTab(tab.id)),
      this.item('Close Other Tabs', () => closeOthers(tabs, tab.id), tabs.list.length > 1),
      this.item('Close Tabs to the Right', () => closeToRight(tabs, tab.id), tabs.list.at(-1) !== tab),
      { type: 'separator' },
      this.item('Reopen Closed Tab', () => reopenClosed(tabs), tabs.closed.size > 0),
    ];
  }
}
