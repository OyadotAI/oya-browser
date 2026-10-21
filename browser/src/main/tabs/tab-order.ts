/**
 * The order of the tab strip and what the strip's own commands do to it:
 * moving a tab, opening one beside another, closing the others or those to
 * the right, and reopening what was closed. The main process owns the order;
 * the shell only asks.
 */
import { CLOSED_TABS_MAX, WEB_URL } from './constants.ts';
import type { LoadURLOptions, Session, WebContents } from 'electron';
import type { Tab, TabStrip } from './types.ts';
import { privateSession } from '../native-contexts/index.ts';

/** Whether a tab is worth bringing back: not the start page, a popup window, a private session or a blank tab. */
export function reopenable(tab: Tab, own?: Session): boolean {
  const session = tab.view?.webContents?.session;
  const foreign = !!session && privateSession(session) && session !== own;
  return !tab.home && !tab.window && !!tab.url && tab.url !== 'about:blank' && !foreign;
}

/** A closed tab worth reopening: where it was, and its place on the strip. */
export interface ClosedTab {
  /** Its address. */
  url: string;
  /** Its place on the strip when it closed. */
  index: number;
}

/** The last tabs a person closed, newest last, so Reopen can bring them back where they were. */
export class ClosedTabs {
  /** The most entries kept; the oldest goes first. */
  private readonly max: number;
  /** The closed tabs, newest last. */
  entries: ClosedTab[] = [];

  /** An empty stack holding at most `max` entries. */
  constructor(max: number = CLOSED_TABS_MAX) {
    this.max = max;
  }

  /** Remembers a closed tab's address and place; the start page and blank tabs are not worth reopening. */
  push(url: string, index: number): void {
    if (!url || url === 'about:blank') return;
    this.entries.push({ url, index });
    if (this.entries.length > this.max) this.entries.shift();
  }

  /** The newest entry, taken off the stack, or undefined. */
  pop(): ClosedTab | undefined {
    return this.entries.pop();
  }

  /** Forgets everything (a profile switch must not reopen the last profile's pages). */
  clear(): void {
    this.entries = [];
  }

  /** How many tabs can be reopened. */
  get size(): number {
    return this.entries.length;
  }
}

/** The tab manager, as the strip's commands use it. */
export interface OrderedTabs extends TabStrip {
  /** Tabs a person closed, for Reopen closed tab. */
  closed: ClosedTabs;
  /** The tab with this id, if open. */
  find(id: number | null): Tab | undefined;
  /** Opens a tab on `url`; returns its id. */
  createTab(
    url: string,
    activate?: boolean,
    options?: import('electron').LoadURLOptions,
    session?: import('electron').Session,
  ): number;
  /** Closes a tab. */
  closeTab(id: number): void;
  /** Shows a tab. */
  activateTab(id: number): void;
}

/** `list` with the item at `from` moved to `to` (clamped to the list), as a new array. */
export function reorder<T>(list: readonly T[], from: number, to: number): T[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(Math.max(0, Math.min(to, next.length)), 0, item);
  return next;
}

/** How many tabs lead the strip pinned: the pinned block always comes first, so this is where it ends. */
export function pinnedCount(list: readonly Tab[]): number {
  const first = list.findIndex((t) => !t.pinned);
  return first === -1 ? list.length : first;
}

/** Whether a person may pin the tab: only a web page the session file keeps, so a pin always comes back after a restart. */
export function pinnable(tab: Tab): boolean {
  return reopenable(tab) && !tab.collection && WEB_URL.test(tab.url);
}

/** `toIndex` kept inside the tab's own group, so a move never lets a pinned tab leave the leading block or an unpinned one enter it. */
function withinGroup(list: readonly Tab[], tab: Tab, toIndex: number): number {
  const pinned = pinnedCount(list);
  const [first, last] = tab.pinned ? [0, pinned - 1] : [pinned, list.length - 1];
  return Math.max(first, Math.min(toIndex, last));
}

/** Puts the strip in `next`'s order and sends it; false when nothing moved. */
function applyOrder(tabs: TabStrip, next: Tab[]): boolean {
  if (next.every((t, i) => t === tabs.list[i])) return false;
  tabs.list.splice(0, tabs.list.length, ...next);
  tabs.sendTabList();
  return true;
}

/** Moves tab `id` to `toIndex` within its group (pinned or not); false when there is no such tab or nothing moved. */
export function moveTabTo(tabs: TabStrip, id: number, toIndex: unknown): boolean {
  const from = tabs.list.findIndex((t) => t.id === id);
  if (from === -1 || typeof toIndex !== 'number' || !Number.isInteger(toIndex)) return false;
  return applyOrder(tabs, reorder(tabs.list, from, withinGroup(tabs.list, tabs.list[from], toIndex)));
}

/**
 * Pins or unpins tab `id`: pinning puts it at the end of the pinned block,
 * unpinning at the start of the rest. The strip is always sent, even when the
 * tab did not move, because the send is what redraws it and saves the session.
 */
export function setPinned(tabs: TabStrip, id: number, pinned: boolean): boolean {
  const from = tabs.list.findIndex((t) => t.id === id);
  if (from === -1 || !!tabs.list[from].pinned === pinned) return false;
  const to = pinnedCount(tabs.list) - (pinned ? 0 : 1);
  tabs.list[from].pinned = pinned;
  if (!applyOrder(tabs, reorder(tabs.list, from, to))) tabs.sendTabList();
  return true;
}

/** Opens `url` in a new tab just right of tab `id`, and shows it; returns the new id. */
export function openBeside(tabs: OrderedTabs, id: number, url: string): number {
  const index = tabs.list.findIndex((t) => t.id === id);
  const created = tabs.createTab(url, true, undefined, tabs.list[index]?.view.webContents.session);
  if (index !== -1) moveTabTo(tabs, created, index + 1);
  return created;
}

/** A link to open in a new tab. */
export interface Link {
  /** The page it was on: its contents name its tab and give the session (a persona or private context) to keep. */
  page: { /** The page's contents. */ readonly webContents: WebContents };
  /** Where it goes. */
  url: string;
  /** Show the new tab; false keeps the person on the page (a background tab). */
  show?: boolean;
  /** How the first load goes (a POST body, a referrer). */
  options?: LoadURLOptions;
}

/** Where a tab opened from the tab at `from` goes: after it and the tabs it already opened beside it, so links opened in a row keep their order, as in Chrome. */
function besideOpener(list: readonly Tab[], from: number): number {
  // A pinned opener's links land after the whole pinned block, so look for its earlier ones from there.
  let index = list[from].pinned ? pinnedCount(list) - 1 : from;
  while (list[index + 1]?.openerId === list[from].id) index++;
  return index + 1;
}

/** Marks tab `id` as opened from the tab at `from`, and moves it right of that tab and its earlier links. */
function placeBeside(tabs: OrderedTabs, from: number, id: number): void {
  const tab = tabs.find(id);
  if (!tab) return;
  tab.openerId = tabs.list[from].id;
  moveTabTo(tabs, id, besideOpener(tabs.list, from));
}

/**
 * Opens a link in a new tab right of the tab it was on, in that page's
 * session. It shows only when asked: "Open Link in New Tab" and Cmd- or
 * middle-clicks open in the background, as in Chrome, so the person stays on
 * the page they were reading. Returns the new id.
 */
export function openLink(tabs: OrderedTabs, { page, url, show = false, options }: Link): number {
  const from = tabs.list.findIndex((t) => t.view.webContents === page.webContents);
  const id = tabs.createTab(url, show, options, page.webContents.session);
  if (from !== -1) placeBeside(tabs, from, id);
  return id;
}

/** Closes every tab `pick` selects from the list, by id, so closing does not shift what is picked. */
function closeWhere(tabs: OrderedTabs, pick: (tab: Tab, index: number) => boolean): void {
  const ids = tabs.list.filter(pick).map((t) => t.id);
  for (const id of ids) tabs.closeTab(id);
}

/** Spares no tab: what a bulk close spares besides pinned tabs, when nothing else is. */
const NONE = (): boolean => false;

/** Closes every unpinned tab but `id`, and shows it: pinned tabs are spared, as in Chrome, and any `spare` picks. */
export function closeOthers(tabs: OrderedTabs, id: number, spare: (tab: Tab) => boolean = NONE): void {
  if (!tabs.find(id)) return;
  closeWhere(tabs, (t) => t.id !== id && !t.pinned && !spare(t));
  tabs.activateTab(id);
}

/** Closes the unpinned tabs right of `id`; pinned tabs are spared, and any `spare` picks. */
export function closeToRight(tabs: OrderedTabs, id: number, spare: (tab: Tab) => boolean = NONE): void {
  const index = tabs.list.findIndex((t) => t.id === id);
  if (index !== -1) closeWhere(tabs, (t, i) => i > index && !t.pinned && !spare(t));
}

/** Reopens the most recently closed tab where it was; returns its id, or undefined when none is left. */
export function reopenClosed(tabs: OrderedTabs): number | undefined {
  const entry = tabs.closed.pop();
  if (!entry) return undefined;
  const id = tabs.createTab(entry.url, true);
  moveTabTo(tabs, id, entry.index);
  return id;
}
