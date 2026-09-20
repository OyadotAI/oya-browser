/**
 * Keyboard shortcuts that work wherever focus is: in the shell, in a tab, or
 * on the control shield. While an agent has control, keys never reach a page.
 */
const { HOME_URL } = require('../tabs/constants.cjs');

/** Cmd/Ctrl + key → shell command. */
const COMMANDS = { l: 'address', k: 'commands', t: 'new-tab', w: 'close-tab' };
/** Cmd/Ctrl + Shift + key → shell command. */
const SHIFT_COMMANDS = { r: 'record', d: 'tools', ']': 'next-tab', '[': 'previous-tab' };
/** Bracket keys by physical code, so they work on every keyboard layout. */
const BRACKETS = { BracketRight: ']', BracketLeft: '[' };

/** Commands the main process runs itself; any other goes to the shell page. */
const LOCAL_COMMANDS = {
  'new-tab': (ctx) => {
    if (!ctx.control.snapshot().interactive) return;
    ctx.tabs.createTab(HOME_URL, true);
    ctx.recorder.recordNavigation(HOME_URL);
  },
  'close-tab': (ctx) => ctx.control.snapshot().interactive && ctx.tabs.closeTab(ctx.tabs.activeTabId),
  'next-tab': (ctx) => ctx.tabs.cycleTab(1),
  'previous-tab': (ctx) => ctx.tabs.cycleTab(ctx.tabs.list.length - 1),
};

/** The shell command an input event asks for, if any. */
function shortcutFor(input) {
  const modifier = process.platform === 'darwin' ? input.meta : input.control;
  if (input.type !== 'keyDown' || !modifier || input.alt) return undefined;
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
