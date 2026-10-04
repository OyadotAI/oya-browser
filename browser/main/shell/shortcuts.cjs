/**
 * Keyboard shortcuts that work wherever focus is: in the shell, in a tab, or
 * on the control shield. While an agent has control, keys never reach a page.
 */
const { HOME_URL } = require('../tabs/constants.cjs');
const { moveTabTo, reopenClosed } = require('../tabs/tab-order.cjs');
const { LAST_TAB_DIGIT } = require('./constants.cjs');

/** Cmd/Ctrl + key → shell command. */
const COMMANDS = { l: 'address', k: 'commands', t: 'new-tab', w: 'close-tab' };
/** Cmd/Ctrl + Shift + key → shell command. */
const SHIFT_COMMANDS = {
  r: 'reload',
  d: 'tools',
  ']': 'next-tab',
  '[': 'previous-tab',
  t: 'reopen-tab',
  pageup: 'move-tab-left',
  pagedown: 'move-tab-right',
};
/**
 * Cmd/Ctrl + Alt + physical key → shell command. Record lives here because
 * Cmd/Ctrl+Shift+R is Chrome's hard reload: people pressed it out of habit and
 * ended their recording. By code, since Alt changes the character on a Mac.
 */
const ALT_COMMANDS = { KeyR: 'record', ArrowRight: 'next-tab', ArrowLeft: 'previous-tab' };
/** Cmd/Ctrl + 1..8 go to that tab and 9 to the last, by physical key so every layout has them. */
const DIGIT = /^Digit([1-9])$/;
/** Bracket keys by physical code, so they work on every keyboard layout. */
const BRACKETS = { BracketRight: ']', BracketLeft: '[' };

/** Shows the `n`th tab (1-based); 9 is always the last, as in Chrome. */
function goToTab(ctx, n) {
  const list = ctx.tabs.list;
  const tab = n === LAST_TAB_DIGIT ? list.at(-1) : list[n - 1];
  if (tab) ctx.tabs.activateTab(tab.id);
}

/** Moves the active tab one place along the strip (Ctrl+Shift+PageUp/PageDown, like Chrome). */
function moveActive(ctx, offset) {
  if (!ctx.control.snapshot().interactive) return;
  const index = ctx.tabs.list.findIndex((t) => t.id === ctx.tabs.activeTabId);
  if (index !== -1 && index + offset >= 0) moveTabTo(ctx.tabs, ctx.tabs.activeTabId, index + offset);
}

/** tab-1 … tab-9 → going to that tab. */
const DIGIT_COMMANDS = Object.fromEntries(
  Array.from({ length: LAST_TAB_DIGIT }, (_v, i) => [`tab-${i + 1}`, (ctx) => goToTab(ctx, i + 1)]),
);

/** Commands the main process runs itself; any other goes to the shell page. */
const LOCAL_COMMANDS = {
  ...DIGIT_COMMANDS,
  'new-tab': (ctx) => {
    if (!ctx.control.snapshot().interactive) return;
    // The start page loads nothing, so a recording has no navigation to keep.
    ctx.tabs.createTab(HOME_URL, true);
  },
  reload: (ctx) => ctx.control.snapshot().interactive && ctx.tabs.reloadActivePage(),
  'close-tab': (ctx) => ctx.control.snapshot().interactive && ctx.tabs.closeTab(ctx.tabs.activeTabId),
  'next-tab': (ctx) => ctx.tabs.cycleTab(1),
  'previous-tab': (ctx) => ctx.tabs.cycleTab(ctx.tabs.list.length - 1),
  'reopen-tab': (ctx) => ctx.control.snapshot().interactive && reopenClosed(ctx.tabs),
  'move-tab-left': (ctx) => moveActive(ctx, -1),
  'move-tab-right': (ctx) => moveActive(ctx, 1),
};

/** Ctrl+Tab and Ctrl+Shift+Tab cycle tabs on every platform, Ctrl even on a Mac, as in every browser. */
function cycleShortcut(input) {
  if (input.key !== 'Tab' || !input.control || input.meta || input.alt) return undefined;
  return input.shift ? 'previous-tab' : 'next-tab';
}

/** The shell command an input event asks for, if any. */
function shortcutFor(input) {
  if (input.type !== 'keyDown' || input.isAutoRepeat) return undefined;
  const modifier = process.platform === 'darwin' ? input.meta : input.control;
  if (!modifier) return cycleShortcut(input);
  if (input.alt) return Object.hasOwn(ALT_COMMANDS, input.code) ? ALT_COMMANDS[input.code] : undefined;
  return keyCommand(input);
}

/** The command for Cmd/Ctrl (+ Shift) and a key, without Alt. */
function keyCommand(input) {
  const digit = DIGIT.exec(input.code || '');
  if (digit && !input.shift) return `tab-${digit[1]}`;
  if (input.key === 'Tab') return cycleShortcut(input);
  const key = Object.hasOwn(BRACKETS, input.code) ? BRACKETS[input.code] : input.key.toLowerCase();
  const table = input.shift ? SHIFT_COMMANDS : COMMANDS;
  return Object.hasOwn(table, key) ? table[key] : undefined;
}

/** Installs the shortcuts on each webContents the shell owns. */
class Shortcuts {
  /** `ctx` is the main-process context (see main.js). */
  constructor(ctx) {
    /** The main-process context. */
    this.ctx = ctx;
  }

  /** Listens to one webContents' keys. */
  install(contents) {
    contents.on('before-input-event', (event, input) => this.onInput(contents, event, input));
  }

  /** Blocks page input while an agent drives, then runs any shortcut. */
  onInput(contents, event, input) {
    const shell = this.ctx.shell.window;
    if (contents !== shell?.webContents && !this.ctx.control.snapshot().interactive) event.preventDefault();
    const command = shortcutFor(input);
    if (!command) return;
    event.preventDefault();
    this.run(command);
  }

  /** Runs a command here, or hands it to the shell page. */
  run(command) {
    if (Object.hasOwn(LOCAL_COMMANDS, command)) return LOCAL_COMMANDS[command](this.ctx);
    this.ctx.shell.window.webContents.focus();
    this.ctx.shell.send('shell-command', command);
  }
}

module.exports = { Shortcuts, shortcutFor };
