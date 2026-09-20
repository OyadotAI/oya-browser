/**
 * Unit tests for the console's shortcut table: tabs, selection moves in
 * table order, Escape clears step by step, and x stops what is selected.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { buildShortcuts, type ShortcutContext } from '@/app/dashboard/_console/shortcuts';

afterEach(() => {
  document.body.innerHTML = '';
});

/** A context with overrides. */
const ctx = (o: Partial<ShortcutContext> = {}): ShortcutContext => ({
  selected: null,
  checked: new Set(),
  overlayOpen: false,
  patch: vi.fn(),
  filterRef: { current: null },
  urlRef: { current: null },
  requestStop: vi.fn(),
  ...o,
});
/** Runs the binding with `keys`. */
const run = (c: ShortcutContext, keys: string) =>
  buildShortcuts(c)
    .find((s) => s.keys === keys)!
    .handler(new KeyboardEvent('keydown'));

describe('console shortcuts', () => {
  it('lists every binding in help-sheet order, the tab switches global', () => {
    const list = buildShortcuts(ctx());
    expect(list.map((s) => s.keys)).toEqual([
      'mod+1',
      'mod+2',
      'mod+3',
      'mod+4',
      '?',
      'n',
      '/',
      'down',
      'up',
      'j',
      'k',
      'escape',
      'x',
      'l',
      'r',
      's',
    ]);
    expect(list.filter((s) => s.global).map((s) => s.keys)).toEqual(['mod+1', 'mod+2', 'mod+3', 'mod+4', 'escape']);
  });

  it('switches tabs', () => {
    const c = ctx();
    run(c, 'mod+4');
    expect(c.patch).toHaveBeenCalledWith({ tab: 'playbooks' });
  });

  it('moves the selection through the rows in table order, stopping at the ends', () => {
    document.body.innerHTML = '<table><tbody><tr data-id="a"></tr><tr data-id="b"></tr></tbody></table>';
    const fresh = ctx();
    run(fresh, 'down');
    expect(fresh.patch).toHaveBeenCalledWith({ selected: 'a' });
    const last = ctx({ selected: 'b' });
    run(last, 'j');
    expect(last.patch).toHaveBeenCalledWith({ selected: 'b' });
    const first = ctx({ selected: 'b' });
    run(first, 'up');
    expect(first.patch).toHaveBeenCalledWith({ selected: 'a' });
  });

  it('Escape closes the panel first, then clears the ticks, and defers to an open dialog', () => {
    const panel = ctx({ selected: 'a', checked: new Set(['b']) });
    run(panel, 'escape');
    expect(panel.patch).toHaveBeenCalledWith({ selected: null });
    const ticks = ctx({ checked: new Set(['b']) });
    run(ticks, 'escape');
    expect(ticks.patch).toHaveBeenCalledWith({ checked: new Set() });
    const dialog = ctx({ selected: 'a', overlayOpen: true });
    run(dialog, 'escape');
    expect(dialog.patch).not.toHaveBeenCalled();
  });

  it('x stops the ticked browsers, else the selected one, else nothing', () => {
    const ticked = ctx({ selected: 'a', checked: new Set(['b', 'c']) });
    run(ticked, 'x');
    expect(ticked.requestStop).toHaveBeenCalledWith(['b', 'c']);
    const one = ctx({ selected: 'a' });
    run(one, 'x');
    expect(one.requestStop).toHaveBeenCalledWith(['a']);
    const none = ctx();
    run(none, 'x');
    expect(none.requestStop).toHaveBeenCalledWith([]);
  });

  it("/ opens the browsers tab and focuses the filter; r and s press the panel's buttons", () => {
    const input = document.body.appendChild(document.createElement('input'));
    const c = ctx({ selected: 'a', filterRef: { current: input } });
    run(c, '/');
    expect(c.patch).toHaveBeenCalledWith({ tab: 'browsers' });
    expect(document.activeElement).toBe(input);
    document.body.insertAdjacentHTML(
      'beforeend',
      '<aside><button title="Reload page">r</button><button>Screenshot</button></aside>',
    );
    const clicks = vi.fn();
    document.querySelectorAll('aside button').forEach((b) => b.addEventListener('click', () => clicks(b.textContent)));
    run(c, 'r');
    run(c, 's');
    run(ctx(), 's');
    expect(clicks.mock.calls).toEqual([['r'], ['Screenshot']]);
  });
});
