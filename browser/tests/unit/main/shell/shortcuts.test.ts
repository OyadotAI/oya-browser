/**
 * Unit tests for the shell's keyboard shortcuts: which keys map to which
 * command, page input blocked while an agent drives, and tab commands.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { Shortcuts, shortcutFor } from '../../../../src/main/shell/shortcuts.ts';
import { sendNativeKey } from '../../../../src/main/input/native-key-dispatch.ts';
import { TabManager } from '../../../../src/main/tabs/tabs.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

const mod = process.platform === 'darwin' ? { meta: true } : { control: true };

/** A keyDown with the platform's command modifier. */
const key = (k, extra = {}) => ({ type: 'keyDown', key: k, code: '', ...mod, ...extra });

describe('shortcutFor', () => {
  it('maps the command-key shortcuts', () => {
    assert.equal(shortcutFor(key('L')), 'address');
    assert.equal(shortcutFor(key('k')), 'commands');
    assert.equal(shortcutFor(key('®', { alt: true, code: 'KeyR' })), 'record');
    assert.equal(shortcutFor(key('}', { shift: true, code: 'BracketRight' })), 'next-tab');
  });

  it("maps Chrome's tab keys: numbers, Ctrl+Tab, Cmd+Option+arrows, reopen and move", () => {
    assert.equal(shortcutFor(key('1', { code: 'Digit1' })), 'tab-1');
    assert.equal(shortcutFor(key('&', { code: 'Digit1' })), 'tab-1', 'by physical key, so AZERTY has it too');
    assert.equal(shortcutFor(key('9', { code: 'Digit9' })), 'tab-9');
    assert.equal(shortcutFor({ type: 'keyDown', key: 'Tab', code: 'Tab', control: true }), 'next-tab');
    assert.equal(shortcutFor({ type: 'keyDown', key: 'Tab', code: 'Tab', control: true, shift: true }), 'previous-tab');
    assert.equal(shortcutFor(key('ArrowRight', { alt: true, code: 'ArrowRight' })), 'next-tab');
    assert.equal(shortcutFor(key('ArrowLeft', { alt: true, code: 'ArrowLeft' })), 'previous-tab');
    assert.equal(shortcutFor(key('T', { shift: true, code: 'KeyT' })), 'reopen-tab');
    assert.equal(shortcutFor(key('PageUp', { shift: true, code: 'PageUp' })), 'move-tab-left');
    assert.equal(shortcutFor(key('PageDown', { shift: true, code: 'PageDown' })), 'move-tab-right');
  });

  it('leaves a plain Tab, and Tab with Alt, to the page', () => {
    assert.equal(shortcutFor({ type: 'keyDown', key: 'Tab', code: 'Tab' }), undefined);
    assert.equal(shortcutFor({ type: 'keyDown', key: 'Tab', code: 'Tab', control: true, alt: true }), undefined);
  });

  it("reloads on Chrome's hard-reload keys instead of toggling the recording", () => {
    assert.equal(shortcutFor(key('r', { shift: true })), 'reload');
  });

  it('ignores a held key repeating, so one press never starts and stops a recording', () => {
    assert.equal(shortcutFor(key('®', { alt: true, code: 'KeyR', isAutoRepeat: true })), undefined);
  });

  it('ignores key-up, alt, a missing modifier and unknown keys', () => {
    assert.equal(shortcutFor({ ...key('l'), type: 'keyUp' }), undefined);
    assert.equal(shortcutFor(key('l', { alt: true })), undefined);
    assert.equal(shortcutFor({ type: 'keyDown', key: 'l', code: '' }), undefined);
    assert.equal(shortcutFor(key('constructor')), undefined);
  });
});

describe('Shortcuts', () => {
  let ctx: any, contents: any;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ shortcuts: Shortcuts, tabs: TabManager });
    contents = new EventEmitter();
    ctx.shortcuts.install(contents);
  });
  afterEach(() => mock.timers.reset());

  /** Sends one key through `contents`; returns whether it was swallowed. */
  const press = (input) => {
    const event = {
      prevented: false,
      preventDefault() {
        this.prevented = true;
      },
    };
    contents.emit('before-input-event', event, input);
    return event.prevented;
  };

  it('allows native agent keys without triggering shell shortcuts, then fences human keys again', () => {
    ctx.control.state.interactive = false;
    contents.sendInputEvent = () => assert.equal(press(key('l')), false);
    sendNativeKey(contents, { type: 'keyDown', keyCode: 'l' });
    assert.equal(press({ type: 'keyDown', key: 'a' }), true);
  });

  it('keeps page keys from the page while an agent has control', () => {
    ctx.control.state.interactive = false;
    assert.equal(press({ type: 'keyDown', key: 'a' }), true);
    ctx.control.state.interactive = true;
    assert.equal(press({ type: 'keyDown', key: 'a' }), false);
  });

  it('hands shell commands to the shell page', () => {
    assert.equal(press(key('k')), true);
    assert.deepEqual(ctx.shell.sentOn('shell-command'), ['commands']);
  });

  it('opens a new tab on the start page only for a person in control, recording no navigation', () => {
    const recorded = mock.method(ctx.recorder, 'recordNavigation', () => {});
    press(key('t'));
    assert.equal(ctx.tabs.list.length, 1);
    assert.ok(ctx.tabs.list[0].home);
    assert.equal(recorded.mock.callCount(), 0);
    ctx.control.state.interactive = false;
    press(key('t'));
    assert.equal(ctx.tabs.list.length, 1);
  });

  it('closes the active tab and moves between tabs', () => {
    ctx.tabs.createTab('https://a.test/');
    ctx.tabs.createTab('https://b.test/');
    press(key('[', { shift: true, code: 'BracketLeft' }));
    assert.equal(ctx.tabs.activeTabId, 1);
    press(key('w'));
    assert.deepEqual(
      ctx.tabs.list.map((t) => t.id),
      [2],
    );
  });

  it('goes to tab N with Cmd/Ctrl+N, and to the last with 9', () => {
    for (const site of ['a', 'b', 'c']) ctx.tabs.createTab(`https://${site}.test/`);
    press(key('1', { code: 'Digit1' }));
    assert.equal(ctx.tabs.activeTabId, 1);
    press(key('9', { code: 'Digit9' }));
    assert.equal(ctx.tabs.activeTabId, 3);
    press(key('8', { code: 'Digit8' }));
    assert.equal(ctx.tabs.activeTabId, 3, 'there is no eighth tab');
  });

  it('reopens the last closed tab where it was, only for a person in control', () => {
    for (const site of ['a', 'b', 'c']) ctx.tabs.createTab(`https://${site}.test/`);
    ctx.tabs.closeTab(2);
    ctx.control.state.interactive = false;
    press(key('T', { shift: true, code: 'KeyT' }));
    assert.equal(ctx.tabs.list.length, 2);
    ctx.control.state.interactive = true;
    press(key('T', { shift: true, code: 'KeyT' }));
    assert.equal(ctx.tabs.list[1].url, 'https://b.test/');
  });

  it('moves the active tab along the strip from the keyboard, stopping at the ends', () => {
    for (const site of ['a', 'b', 'c']) ctx.tabs.createTab(`https://${site}.test/`);
    press(key('PageUp', { shift: true, code: 'PageUp' }));
    assert.deepEqual(
      ctx.tabs.list.map((t) => t.id),
      [1, 3, 2],
    );
    press(key('PageUp', { shift: true, code: 'PageUp' }));
    press(key('PageUp', { shift: true, code: 'PageUp' }));
    assert.deepEqual(
      ctx.tabs.list.map((t) => t.id),
      [3, 1, 2],
    );
    press(key('PageDown', { shift: true, code: 'PageDown' }));
    assert.deepEqual(
      ctx.tabs.list.map((t) => t.id),
      [1, 3, 2],
    );
  });
});
