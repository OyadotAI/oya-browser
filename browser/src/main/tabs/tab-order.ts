/**
 * The order of the tab strip and what the strip's own commands do to it:
 * moving a tab, opening one beside another, closing the others or those to
 * the right, and reopening what was closed. The main process owns the order;
 * the shell only asks.
 */
import { CLOSED_TABS_MAX } from './constants.ts';
import type { Tab, TabStrip } from './types.ts';

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
  createTab(url: string, activate?: boolean): number;
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

/** Moves tab `id` to `toIndex` on the strip; false when there is no such tab or nothing moved. */
export function moveTabTo(tabs: TabStrip, id: number, toIndex: unknown): boolean {
  const from = tabs.list.findIndex((t) => t.id === id);
  if (from === -1 || typeof toIndex !== 'number' || !Number.isInteger(toIndex)) return false;
  const next = reorder(tabs.list, from, toIndex);
  if (next.every((t, i) => t === tabs.list[i])) return false;
  tabs.list.splice(0, tabs.list.length, ...next);
  tabs.sendTabList();
  return true;
}

/** Opens `url` in a new tab just right of tab `id`, and shows it; returns the new id. */
export function openBeside(tabs: OrderedTabs, id: number, url: string): number {
  const index = tabs.list.findIndex((t) => t.id === id);
  const created = tabs.createTab(url, true);
  if (index !== -1) moveTabTo(tabs, created, index + 1);
  return created;
}

/** Closes every tab `pick` selects from the list, by id, so closing does not shift what is picked. */
function closeWhere(tabs: OrderedTabs, pick: (tab: Tab, index: number) => boolean): void {
  const ids = tabs.list.filter(pick).map((t) => t.id);
  for (const id of ids) tabs.closeTab(id);
}

/** Closes every tab but `id`, and shows it. */
export function closeOthers(tabs: OrderedTabs, id: number): void {
  if (!tabs.find(id)) return;
  closeWhere(tabs, (t) => t.id !== id);
  tabs.activateTab(id);
}

/** Closes the tabs right of `id`. */
export function closeToRight(tabs: OrderedTabs, id: number): void {
  const index = tabs.list.findIndex((t) => t.id === id);
  if (index !== -1) closeWhere(tabs, (_t, i) => i > index);
}

/** Reopens the most recently closed tab where it was; returns its id, or undefined when none is left. */
export function reopenClosed(tabs: OrderedTabs): number | undefined {
  const entry = tabs.closed.pop();
  if (!entry) return undefined;
  const id = tabs.createTab(entry.url, true);
  moveTabTo(tabs, id, entry.index);
  return id;
}
