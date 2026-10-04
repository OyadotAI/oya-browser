/**
 * The command palette: searchable shell commands with their shortcuts, and
 * the shortcuts the main process forwards from the menu. What another feature
 * owns (the address bar, the Ask box, recording) is reached through
 * `PaletteHost`, which the composition root wires.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { RendererServices } from '../../../app/services.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ShellDialogViewModel } from './shell-dialog-view-model.ts';
import type { UpdatesViewModel } from './updates-view-model.ts';

/** What other features do for the palette. */
export interface PaletteHost {
  /** Focuses and selects the address bar (the navigation toolbar). */
  focusAddress(): void;
  /** Focuses the Ask box (the chat pane), once the panel shows it. */
  focusChat(): void;
  /** Presses the studio's record button, through the control guard (the studio). */
  recordButton(): void;
}

/** A command's name. */
export type CommandId = 'address' | 'newTab' | 'ask' | 'record' | 'inspect' | 'account' | 'updates';

/** One command as the palette lists it. */
export interface Command {
  /** What runs it. */
  id: CommandId;
  /** Its words. */
  label: string;
  /** Its shortcut with this platform's modifier ('' for none). */
  shortcut: string;
}

/** The palette's commands: id, label, and keys after the platform modifier ('' for none). */
const COMMANDS: readonly (readonly [CommandId, string, string])[] = [
  ['address', 'Focus address bar', 'L'],
  ['newTab', 'New tab', 'T'],
  ['ask', 'Ask Oya', '⇧ D'],
  ['record', 'Record a workflow', '⌥ R'],
  ['inspect', 'Inspect this page', ''],
  ['account', 'Account and connection', ''],
  ['updates', 'Check for updates', ''],
];

/** The shortcuts the main process forwards from the menu. */
export type ShortcutName = 'address' | 'commands' | 'record' | 'tools';

/** What the palette shows. */
export interface PaletteState {
  /** The search as typed. */
  query: string;
}

/** Every command for `platform` (navigator.platform): ⌘ on a Mac, Ctrl elsewhere. */
export function commandsFor(platform: string): Command[] {
  const modifier = platform.includes('Mac') ? '⌘' : 'Ctrl';
  return COMMANDS.map(([id, label, keys]) => ({ id, label, shortcut: keys && `${modifier} ${keys}` }));
}

/** The commands whose label holds `query`, ignoring case. */
export function matching(commands: Command[], query: string): Command[] {
  const q = query.toLowerCase();
  return commands.filter((command) => command.label.toLowerCase().includes(q));
}

/** What the palette needs from its neighbours. */
export interface PaletteDeps extends Pick<RendererServices, 'panel' | 'shell'> {
  /** The main process. */
  bridge: Pick<OyaBrowser, 'newTab' | 'onShellCommand'>;
  /** The dialog the palette lives in. */
  dialog: Pick<ShellDialogViewModel, 'open' | 'close' | 'state' | 'subscribe'>;
  /** Check for updates. */
  updates: Pick<UpdatesViewModel, 'check'>;
  /** Other features. */
  host: PaletteHost;
  /** navigator.platform. */
  platform: string;
}

/** The palette. */
export class PaletteViewModel extends ViewModel<PaletteState> {
  /** Its neighbours. */
  private readonly deps: PaletteDeps;
  /** Every command, for this platform. */
  readonly commands: Command[];
  /** What each command does. */
  private readonly actions: Record<CommandId, () => unknown>;
  /** What each forwarded shortcut does. */
  private readonly shortcuts: Record<ShortcutName, () => unknown>;

  /** An empty search, fresh each time the dialog opens on the commands; listens for menu shortcuts. */
  constructor(deps: PaletteDeps) {
    super({ query: '' });
    this.deps = deps;
    this.commands = commandsFor(deps.platform);
    this.actions = this.commandActions();
    this.shortcuts = this.shortcutActions();
    this.own(deps.bridge.onShellCommand((name) => this.shortcut(name)));
    this.own(this.resetOnOpen());
  }

  /** The search changed. */
  search(query: string): void {
    this.set({ query });
  }

  /** The commands matching the search. */
  get matches(): Command[] {
    return matching(this.commands, this.state.query);
  }

  /** Closes the dialog, then runs the command. */
  async run(id: CommandId): Promise<void> {
    this.deps.dialog.close();
    await this.actions[id]();
  }

  /** Enter in the search: runs the first match, if any. */
  async runFirst(): Promise<void> {
    const first = this.matches[0];
    if (first) await this.run(first.id);
  }

  /** Runs a forwarded shortcut; unknown ones are ignored. */
  shortcut(name: string): void {
    if (Object.hasOwn(this.shortcuts, name)) void this.shortcuts[name as ShortcutName]();
  }

  /** What each command does. */
  private commandActions(): Record<CommandId, () => unknown> {
    const { bridge, dialog, updates } = this.deps;
    return {
      ...this.hostActions(),
      ...this.paneActions(),
      newTab: () => bridge.newTab(),
      account: () => dialog.open(true),
      updates: () => updates.check(),
    };
  }

  /** The commands that open the workspace panel: Ask (then its box takes focus) and Inspect. */
  private paneActions(): Record<'ask' | 'inspect', () => unknown> {
    const { panel, host } = this.deps;
    return { ask: () => panel.open('chat').then(() => host.focusChat()), inspect: () => panel.open('actions') };
  }

  /** What each forwarded shortcut does. */
  private shortcutActions(): Record<ShortcutName, () => unknown> {
    const { panel, shell, dialog } = this.deps;
    return {
      ...this.hostActions(),
      commands: () => dialog.open(),
      tools: () => panel.toggle(shell.state.recording),
    };
  }

  /** What a command and a shortcut both do: the address bar, and the record button. */
  private hostActions(): Record<'address' | 'record', () => unknown> {
    const { host } = this.deps;
    return { address: () => host.focusAddress(), record: () => host.recordButton() };
  }

  /** Clears the search whenever the dialog opens on the commands. */
  private resetOnOpen(): () => void {
    let wasOpen = false;
    return this.deps.dialog.subscribe(() => {
      const { open, page } = this.deps.dialog.state;
      if (open && !wasOpen && page === 'commands') this.set({ query: '' });
      wasOpen = open;
    });
  }
}
