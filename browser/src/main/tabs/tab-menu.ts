/**
 * The tab strip's right-click menu, a native menu like Chrome's: new tab to
 * the right, reload, duplicate, pin or unpin, close, close others, close to
 * the right, and reopen closed tab. Every item acts only while the person may use
 * the tab, and the bulk closes leave alone any tab an agent is working in.
 */
import type { MenuItemConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import { HOME_URL } from './constants.ts';
import { openBeside, closeOthers, closeToRight, reopenClosed, pinnable, setPinned } from './tab-order.ts';
import type { Tab } from './types.ts';
import { reloadOrRecover } from './tab-recovery.ts';

/** The services the menu uses. */
type Deps = Pick<AppServices, 'agentTab' | 'tabs' | 'control' | 'electron' | 'shell' | 'windows'>;

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

  /**
   * One item that runs `run` only while the person may use tab `tab`. This is the
   * per-tab check the tab's close button uses: a desktop rests in agent mode, so
   * asking for whole-browser control left every item dead even with no agent running.
   */
  private item(tab: Tab, label: string, run: () => unknown, enabled = true): MenuItemConstructorOptions {
    return { label, enabled, click: () => this.deps.agentTab.humanMay(tab.id) && run() };
  }

  /** The items that open or reload: they act on tab `tab`. */
  private openItems(tab: Tab): MenuItemConstructorOptions[] {
    const tabs = this.deps.tabs;
    return [
      this.item(tab, 'New Tab to the Right', () => openBeside(tabs, tab.id, HOME_URL)),
      { type: 'separator' },
      this.item(tab, 'Reload', () => reloadOrRecover(this.deps, tab, () => tabs.reloadTab(tab)), !tab.home),
      this.item(tab, 'Duplicate', () => openBeside(tabs, tab.id, tab.url || HOME_URL)),
      ...this.placeItems(tab),
    ];
  }

  /** Pin or Unpin, and Move to a new window: the items that change where the tab lives. */
  private placeItems(tab: Tab): MenuItemConstructorOptions[] {
    return [this.pinItem(tab), this.moveItem(tab)];
  }

  /** Moving a person's tab does not interrupt an agent working in another one. */
  private moveItem(tab: Tab): MenuItemConstructorOptions {
    return {
      label: 'Move Tab to New Window',
      enabled: !!this.deps.windows && !tab.window && this.deps.agentTab.humanMay(tab.id),
      click: () => this.deps.agentTab.humanMay(tab.id) && this.deps.windows?.detach(tab.id),
    };
  }

  /**
   * Pin or Unpin. A person may not pin a tab an agent is working in, nor one
   * the session file would drop (start page, collection, popup, private), since
   * that pin could never come back after a restart.
   */
  private pinItem(tab: Tab): MenuItemConstructorOptions {
    const may = () => this.deps.agentTab.humanMay(tab.id) && (!!tab.pinned || pinnable(tab));
    return {
      label: tab.pinned ? 'Unpin Tab' : 'Pin Tab',
      enabled: may(),
      click: () => may() && setPinned(this.deps.tabs, tab.id, !tab.pinned),
    };
  }

  /** The items that close, and Reopen closed tab. */
  private closeItems(tab: Tab): MenuItemConstructorOptions[] {
    const tabs = this.deps.tabs;
    return [
      this.item(tab, 'Close Tab', () => tabs.closeTab(tab.id)),
      ...this.bulkCloseItems(tab),
      { type: 'separator' },
      this.item(tab, 'Reopen Closed Tab', () => reopenClosed(tabs), tabs.closed.size > 0),
    ];
  }

  /** Close Other Tabs and Close Tabs to the Right: each spares pinned tabs, so each is on only when an unpinned tab would close. */
  private bulkCloseItems(tab: Tab): MenuItemConstructorOptions[] {
    const tabs = this.deps.tabs;
    const index = tabs.list.indexOf(tab);
    const others = this.unpinned((t) => t !== tab);
    const right = this.unpinned((_t, i) => i > index);
    return [
      this.item(tab, 'Close Other Tabs', () => closeOthers(tabs, tab.id, this.spared()), others),
      this.item(tab, 'Close Tabs to the Right', () => closeToRight(tabs, tab.id, this.spared()), right),
    ];
  }

  /** The tabs a bulk close leaves alone besides pinned ones: those agents are working in. */
  private spared(): (tab: Tab) => boolean {
    const driven = this.deps.agentTab.claimed();
    return (t) => driven.includes(t.id);
  }

  /** Whether some unpinned tab matches `pick`: the bulk closes spare pinned tabs, so only these give them something to do. */
  private unpinned(pick: (tab: Tab, index: number) => boolean): boolean {
    return this.deps.tabs.list.some((t, i) => !t.pinned && pick(t, i));
  }
}
