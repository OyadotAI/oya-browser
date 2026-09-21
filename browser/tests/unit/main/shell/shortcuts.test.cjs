/**
 * Unit tests for the shell's keyboard shortcuts: which keys map to which
 * command, page input blocked while an agent drives, and tab commands.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { Shortcuts, shortcutFor } = require('../../../../main/shell/shortcuts.cjs');
const { TabManager } = require('../../../../main/tabs/tabs.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

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
  let ctx, contents;
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

  it('opens and records a new tab only for a person in control', () => {
    const recorded = mock.method(ctx.recorder, 'recordNavigation', () => {});
    press(key('t'));
    assert.equal(ctx.tabs.list.length, 1);
    assert.equal(recorded.mock.callCount(), 1);
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
});
