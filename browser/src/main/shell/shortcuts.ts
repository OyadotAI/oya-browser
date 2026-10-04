/**
 * Keyboard shortcuts that work wherever focus is: in the shell, in a tab, or
 * on the control shield. While an agent has control, keys never reach a page.
 */
import type { Event as ElectronEvent, Input, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { moveTabTo, reopenClosed } from '../tabs/tab-order.ts';
import { LAST_TAB_DIGIT } from './constants.ts';

/** The services the shortcuts use. */
type Deps = Pick<AppServices, 'shell' | 'control' | 'tabs'>;

/** A command the main process runs itself. */
type LocalCommand = (deps: Deps) => unknown;

/** The parts of a key event the shortcuts read. */
export type KeyInput = Pick<Input, 'type' | 'key' | 'code' | 'isAutoRepeat' | 'shift' | 'control' | 'alt' | 'meta'>;

/** A tab in the strip, as far as the shortcuts care. */
interface TabRef {
  /** The tab's id. */
  id: number;
}

/** Cmd/Ctrl + key to shell command. */
const COMMANDS: Record<string, string> = { l: 'address', k: 'commands', t: 'new-tab', w: 'close-tab' };
/** Cmd/Ctrl + Shift + key to shell command. */
const SHIFT_COMMANDS: Record<string, string> = {
  r: 'reload',
  d: 'tools',
  ']': 'next-tab',
  '[': 'previous-tab',
  t: 'reopen-tab',
  pageup: 'move-tab-left',
  pagedown: 'move-tab-right',
};
/**
 * Cmd/Ctrl + Alt + physical key to shell command. Record lives here because
 * Cmd/Ctrl+Shift+R is Chrome's hard reload: people pressed it out of habit and
 * ended their recording. By code, since Alt changes the character on a Mac.
 */
const ALT_COMMANDS: Record<string, string> = { KeyR: 'record', ArrowRight: 'next-tab', ArrowLeft: 'previous-tab' };
/** Cmd/Ctrl + 1..8 go to that tab and 9 to the last, by physical key so every layout has them. */
const DIGIT = /^Digit([1-9])$/;
/** Bracket keys by physical code, so they work on every keyboard layout. */
const BRACKETS: Record<string, string> = { BracketRight: ']', BracketLeft: '[' };

/** Shows the `n`th tab (1-based); 9 is always the last, as in Chrome. */
function goToTab(deps: Deps, n: number): void {
  const list = deps.tabs.list;
  const tab = n === LAST_TAB_DIGIT ? list.at(-1) : list[n - 1];
  if (tab) deps.tabs.activateTab(tab.id);
}

/** Moves the active tab one place along the strip (Ctrl+Shift+PageUp/PageDown, like Chrome). */
function moveActive(deps: Deps, offset: number): void {
  if (!deps.control.snapshot().interactive) return;
  const index = deps.tabs.list.findIndex((t: TabRef) => t.id === deps.tabs.activeTabId);
  if (index !== -1 && index + offset >= 0) moveTabTo(deps.tabs, deps.tabs.activeTabId!, index + offset);
}

/** tab-1 ... tab-9 to going to that tab. */
const DIGIT_COMMANDS: Record<string, LocalCommand> = Object.fromEntries(
  Array.from({ length: LAST_TAB_DIGIT }, (_v, i) => [`tab-${i + 1}`, (deps: Deps) => goToTab(deps, i + 1)]),
);

/** Commands the main process runs itself; any other goes to the shell page. */
const LOCAL_COMMANDS: Record<string, LocalCommand> = {
  ...DIGIT_COMMANDS,
  'new-tab': (deps) => {
    if (!deps.control.snapshot().interactive) return;
    // The start page loads nothing, so a recording has no navigation to keep.
    deps.tabs.createTab(HOME_URL, true);
  },
  reload: (deps) => deps.control.snapshot().interactive && deps.tabs.reloadActivePage(),
  'close-tab': (deps) => deps.control.snapshot().interactive && deps.tabs.closeTab(deps.tabs.activeTabId!),
  'next-tab': (deps) => deps.tabs.cycleTab(1),
  'previous-tab': (deps) => deps.tabs.cycleTab(deps.tabs.list.length - 1),
  'reopen-tab': (deps) => deps.control.snapshot().interactive && reopenClosed(deps.tabs),
  'move-tab-left': (deps) => moveActive(deps, -1),
  'move-tab-right': (deps) => moveActive(deps, 1),
};

/** Ctrl+Tab and Ctrl+Shift+Tab cycle tabs on every platform, Ctrl even on a Mac, as in every browser. */
function cycleShortcut(input: KeyInput): string | undefined {
  if (input.key !== 'Tab' || !input.control || input.meta || input.alt) return undefined;
  return input.shift ? 'previous-tab' : 'next-tab';
}

/** The shell command an input event asks for, if any. */
export function shortcutFor(input: KeyInput): string | undefined {
  if (input.type !== 'keyDown' || input.isAutoRepeat) return undefined;
  const modifier = process.platform === 'darwin' ? input.meta : input.control;
  if (!modifier) return cycleShortcut(input);
  if (input.alt) return Object.hasOwn(ALT_COMMANDS, input.code) ? ALT_COMMANDS[input.code] : undefined;
  return keyCommand(input);
}

/** The command for Cmd/Ctrl (+ Shift) and a key, without Alt. */
function keyCommand(input: KeyInput): string | undefined {
  const digit = DIGIT.exec(input.code || '');
  if (digit && !input.shift) return `tab-${digit[1]}`;
  if (input.key === 'Tab') return cycleShortcut(input);
  const key = Object.hasOwn(BRACKETS, input.code) ? BRACKETS[input.code] : input.key.toLowerCase();
  const table = input.shift ? SHIFT_COMMANDS : COMMANDS;
  return Object.hasOwn(table, key) ? table[key] : undefined;
}

/** Installs the shortcuts on each webContents the shell owns. */
export class Shortcuts {
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process context (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Listens to one webContents' keys. */
  install(contents: WebContents): void {
    contents.on('before-input-event', (event, input) => this.onInput(contents, event, input));
  }

  /** Blocks page input while an agent drives, then runs any shortcut. */
  private onInput(contents: WebContents, event: ElectronEvent, input: KeyInput): void {
    const shell = this.deps.shell.window;
    if (contents !== shell?.webContents && !this.deps.control.snapshot().interactive) event.preventDefault();
    const command = shortcutFor(input);
    if (!command) return;
    event.preventDefault();
    this.run(command);
  }

  /** Runs a command here, or hands it to the shell page. */
  private run(command: string): void {
    if (Object.hasOwn(LOCAL_COMMANDS, command)) {
      LOCAL_COMMANDS[command](this.deps);
      return;
    }
    this.deps.shell.window!.webContents.focus();
    this.deps.shell.send('shell-command', command);
  }
}
