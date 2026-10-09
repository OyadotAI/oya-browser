/**
 * Keyboard shortcuts that work wherever focus is: in the shell, in a tab, or
 * on the control shield. While an agent has control, human keys cannot reach a
 * page; browser-owned native key dispatch bypasses shortcuts, not ownership.
 */
import type { Event as ElectronEvent, Input, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { isNativeKeyDispatch } from '../input/index.ts';
import { LibraryMenu } from '../library/index.ts';
import { HOME_URL } from '../tabs/constants.ts';
import { moveTabTo, reopenClosed } from '../tabs/tab-order.ts';
import { resolveShortcut } from '../../shared/shortcuts.ts';
import { LAST_TAB_DIGIT, ZOOM_STEP } from './constants.ts';

/** The services the shortcuts use. */
type Deps = Pick<AppServices, 'shell' | 'control' | 'tabs' | 'library' | 'electron' | 'persona' | 'windows'>;

/** A command the main process runs itself. */
type LocalCommand = (deps: Deps) => unknown;

/** The parts of a key event the shortcuts read. */
export type KeyInput = Pick<Input, 'type' | 'key' | 'code' | 'isAutoRepeat' | 'shift' | 'control' | 'alt' | 'meta'>;

/** A tab in the strip, as far as the shortcuts care. */
interface TabRef {
  /** The tab's id. */
  id: number;
}

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

/** Zoom only the page: never distort the shell relative to its native view bounds. */
function zoomPage(deps: Deps, offset: number | null): void {
  if (!deps.control.snapshot().interactive) return;
  const contents = deps.tabs.getActiveView()?.webContents;
  if (contents) contents.setZoomLevel(offset === null ? 0 : contents.getZoomLevel() + offset);
}

/** Commands the main process runs itself; any other goes to the shell page. */
const LOCAL_COMMANDS: Record<string, LocalCommand> = {
  ...DIGIT_COMMANDS,
  'new-window': (deps) => deps.control.snapshot().interactive && deps.windows?.newWindow(),
  'detach-tab': (deps) => deps.control.snapshot().interactive && deps.windows?.detach(deps.tabs.activeTabId!),
  'zoom-in': (deps) => zoomPage(deps, ZOOM_STEP),
  'zoom-out': (deps) => zoomPage(deps, -ZOOM_STEP),
  'zoom-reset': (deps) => zoomPage(deps, null),
  library: (deps) => new LibraryMenu(deps).show(),
  bookmark: (deps) => new LibraryMenu(deps).toggle(),
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

/** The shared registry is also the guide's source of truth. */
export function shortcutFor(input: KeyInput): string | undefined {
  return resolveShortcut(input, process.platform);
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
    contents.on('before-input-event', (event, input) => {
      if (!isNativeKeyDispatch(contents)) this.onInput(contents, event, input);
    });
  }

  /** Blocks page input while an agent drives, then runs any shortcut. */
  private onInput(contents: WebContents, event: ElectronEvent, input: KeyInput): void {
    const owner = this.deps.windows?.ownerOfContents(contents);
    if (owner && owner.shortcuts !== this) return owner.shortcuts.onInput(contents, event, input);
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
