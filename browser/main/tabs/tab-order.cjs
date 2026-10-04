/**
 * The order of the tab strip and what the strip's own commands do to it:
 * moving a tab, opening one beside another, closing the others or those to
 * the right, and reopening what was closed. The main process owns the order;
 * the shell only asks.
 */
const { CLOSED_TABS_MAX } = require('./constants.cjs');

/** The last tabs a person closed, newest last, so Reopen can bring them back where they were. */
class ClosedTabs {
  /** An empty stack holding at most `max` entries. */
  constructor(max = CLOSED_TABS_MAX) {
    /** The most entries kept; the oldest goes first. */
    this.max = max;
    /** `{ url, index }` entries, newest last. */
    this.entries = [];
  }

  /** Remembers a closed tab's address and place; the start page and blank tabs are not worth reopening. */
  push(url, index) {
    if (!url || url === 'about:blank') return;
    this.entries.push({ url, index });
    if (this.entries.length > this.max) this.entries.shift();
  }

  /** The newest entry, taken off the stack, or undefined. */
  pop() {
    return this.entries.pop();
  }

  /** Forgets everything (a profile switch must not reopen the last profile's pages). */
  clear() {
    this.entries = [];
  }

  /** How many tabs can be reopened. */
  get size() {
    return this.entries.length;
  }
}

/** `list` with the item at `from` moved to `to` (clamped to the list), as a new array. */
function reorder(list, from, to) {
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(Math.max(0, Math.min(to, next.length)), 0, item);
  return next;
}

/** Moves tab `id` to `toIndex` on the strip; false when there is no such tab or nothing moved. */
function moveTabTo(tabs, id, toIndex) {
  const from = tabs.list.findIndex((t) => t.id === id);
  if (from === -1 || !Number.isInteger(toIndex)) return false;
  const next = reorder(tabs.list, from, toIndex);
  if (next.every((t, i) => t === tabs.list[i])) return false;
  tabs.list.splice(0, tabs.list.length, ...next);
  tabs.sendTabList();
  return true;
}

/** Opens `url` in a new tab just right of tab `id`, and shows it; returns the new id. */
function openBeside(tabs, id, url) {
  const index = tabs.list.findIndex((t) => t.id === id);
  const created = tabs.createTab(url, true);
  if (index !== -1) moveTabTo(tabs, created, index + 1);
  return created;
}

/** Closes every tab `pick` selects from the list, by id, so closing does not shift what is picked. */
function closeWhere(tabs, pick) {
  const ids = tabs.list.filter(pick).map((t) => t.id);
  for (const id of ids) tabs.closeTab(id);
}

/** Closes every tab but `id`, and shows it. */
function closeOthers(tabs, id) {
  if (!tabs.find(id)) return;
  closeWhere(tabs, (t) => t.id !== id);
  tabs.activateTab(id);
}

/** Closes the tabs right of `id`. */
function closeToRight(tabs, id) {
  const index = tabs.list.findIndex((t) => t.id === id);
  if (index !== -1) closeWhere(tabs, (_t, i) => i > index);
}

/** Reopens the most recently closed tab where it was; returns its id, or undefined when none is left. */
function reopenClosed(tabs) {
  const entry = tabs.closed.pop();
  if (!entry) return undefined;
  const id = tabs.createTab(entry.url, true);
  moveTabTo(tabs, id, entry.index);
  return id;
}

module.exports = { ClosedTabs, reorder, moveTabTo, openBeside, closeOthers, closeToRight, reopenClosed };
