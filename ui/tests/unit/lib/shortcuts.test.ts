/**
 * Unit tests for console shortcuts: bindings match keys and modifiers, keys
 * typed into fields stay there, and open overlays or the live view own the
 * keyboard.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, renderHook } from '@testing-library/react';
import { keyCaps, useShortcuts, type Shortcut } from '@/lib/shortcuts';

afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
});

/** A binding that records its calls. */
const bind = (keys: string, global = false): Shortcut => ({
  keys,
  label: keys,
  group: 'Fleet',
  global,
  handler: vi.fn(),
});

/** Dispatches a keydown from `target`. */
function press(key: string, init: KeyboardEventInit = {}, target: EventTarget = document.body) {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(e);
  return e;
}

describe('useShortcuts', () => {
  it('runs the matching binding and prevents the default', () => {
    const n = bind('n');
    renderHook(() => useShortcuts([n]));
    const e = press('n');
    expect(n.handler).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('matches mod as Ctrl off a Mac, and requires exactly the modifiers asked for', () => {
    const one = bind('mod+1');
    renderHook(() => useShortcuts([one]));
    press('1');
    press('1', { ctrlKey: true, altKey: true });
    expect(one.handler).not.toHaveBeenCalled();
    press('1', { ctrlKey: true });
    expect(one.handler).toHaveBeenCalledOnce();
  });

  it("does not require shift twice for '?'", () => {
    const help = bind('shift+?');
    renderHook(() => useShortcuts([help]));
    press('?');
    expect(help.handler).toHaveBeenCalledOnce();
  });

  it('maps up and down to the arrow keys', () => {
    const up = bind('up');
    const down = bind('down');
    renderHook(() => useShortcuts([up, down]));
    press('ArrowUp');
    press('ArrowDown');
    expect(up.handler).toHaveBeenCalledOnce();
    expect(down.handler).toHaveBeenCalledOnce();
  });

  it('leaves keys typed into a field alone unless the binding is global', () => {
    const n = bind('n');
    const esc = bind('escape', true);
    renderHook(() => useShortcuts([n, esc]));
    const input = document.body.appendChild(document.createElement('input'));
    press('n', {}, input);
    press('Escape', {}, input);
    expect(n.handler).not.toHaveBeenCalled();
    expect(esc.handler).toHaveBeenCalledOnce();
  });

  it('gives every key to the live view while it captures the keyboard, Escape included', () => {
    const esc = bind('escape', true);
    renderHook(() => useShortcuts([esc]));
    const view = document.body.appendChild(document.createElement('div'));
    view.setAttribute('data-captures-keys', '');
    press('Escape', {}, view);
    expect(esc.handler).not.toHaveBeenCalled();
  });

  it('does nothing while a dialog or menu is open, or when the key was already handled', () => {
    const n = bind('n');
    renderHook(() => useShortcuts([n]));
    const e = new KeyboardEvent('keydown', { key: 'n', bubbles: true, cancelable: true });
    e.preventDefault();
    document.body.dispatchEvent(e);
    document.body.appendChild(document.createElement('div')).setAttribute('role', 'dialog');
    press('n');
    expect(n.handler).not.toHaveBeenCalled();
  });

  it('listens only while enabled, and stops on unmount', () => {
    const n = bind('n');
    const { rerender, unmount } = renderHook(({ on }) => useShortcuts([n], on), { initialProps: { on: false } });
    press('n');
    rerender({ on: true });
    press('n');
    unmount();
    press('n');
    expect(n.handler).toHaveBeenCalledOnce();
  });
});

describe('keyCaps', () => {
  it('draws modifiers and named keys as caps, and single keys uppercased', () => {
    expect(keyCaps('mod+shift+k')).toEqual(['Ctrl', '⇧', 'K']);
    expect(keyCaps('Escape')).toEqual(['Esc']);
    expect(keyCaps('enter')).toEqual(['↵']);
    expect(keyCaps('up')).toEqual(['↑']);
    expect(keyCaps('alt+Tab')).toEqual(['Alt', 'Tab']);
  });
});
