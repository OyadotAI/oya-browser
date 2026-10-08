/**
 * The command palette: searchable shell commands with their shortcuts, and
 * the shortcuts the main process forwards from the menu. What another feature
 * owns (the address bar, the Ask box, recording) is reached through
 * `PaletteHost`, which the composition root wires.
 */
import { commandShortcut } from '../../../../shared/shortcuts.ts';
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
  /** Opens the shared model editor. */
  openModels?(): void;
}

/** A command's name. */
export type CommandId =
  | 'defaultBrowser'
  | 'shortcuts'
  | 'library'
  | 'back'
  | 'forward'
  | 'address'
  | 'newTab'
  | 'ask'
  | 'record'
  | 'network'
  | 'source'
  | 'inspect'
  | 'account'
  | 'updates'
  | 'playbooks'
  | 'models'
  | 'routines'
  | 'profiles'
  | 'imports';

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
  ['address', 'Focus address bar', 'address'],
  ['newTab', 'New tab', 'new-tab'],
  ['ask', 'Ask Oya', 'ask'],
  ['record', 'Record a workflow', 'record'],
  ['inspect', 'Inspect this page', 'inspect'],
  ['network', 'Network activity', 'network'],
  ['source', 'Page source', 'source'],
  ['account', 'Account and connection', 'account'],
  ['playbooks', 'Playbooks — saved workflows', 'playbooks'],
  ['models', 'Settings — Models', 'models'],
  ['routines', 'Routines', 'routines'],
  ['profiles', 'Profiles and logins', ''],
  ['imports', 'Import browser logins', ''],
  ['defaultBrowser', 'Make Oya Browser default…', ''],
  ['shortcuts', 'Keyboard shortcuts', 'shortcuts'],
  ['library', 'History and bookmarks', 'library'],
  ['back', 'Go back', 'back'],
  ['forward', 'Go forward', 'forward'],
  ['updates', 'Check for updates', ''],
];

/** The shortcuts the main process forwards from the menu. */
export type ShortcutName = CommandId | 'commands' | 'tools';

/** What the palette shows. */
export interface PaletteState {
  /** The search as typed. */
  query: string;
}

/** Every command for `platform` (navigator.platform): ⌘ on a Mac, Ctrl elsewhere. */
export function commandsFor(platform: string): Command[] {
  return COMMANDS.map(([id, label, command]) => ({ id, label, shortcut: commandShortcut(command, platform) }));
}

/** The commands whose label holds `query`, ignoring case. */
export function matching(commands: Command[], query: string): Command[] {
  const q = query.toLowerCase();
  return commands.filter((command) => command.label.toLowerCase().includes(q));
}

/** What the palette needs from its neighbours. */
export interface PaletteDeps extends Pick<RendererServices, 'panel' | 'shell'> {
  /** The main process. */
  bridge: Pick<OyaBrowser, 'makeDefaultBrowser' | 'newTab' | 'onShellCommand' | 'showLibrary' | 'goBack' | 'goForward'>;
  /** The dialog the palette lives in. */
  dialog: Pick<ShellDialogViewModel, 'open' | 'openShortcuts' | 'close' | 'state' | 'subscribe'>;
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

  /** Platform spelling for the guide's keycaps. */
  get platform(): string {
    return this.deps.platform;
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
      ...Object.assign({}, this.hostActions(), this.paneActions(), this.libraryActions(), this.browsingActions()),
      newTab: () => bridge.newTab(),
      account: () => dialog.open(true),
      updates: () => updates.check(),
    };
  }

  /** Navigation uses the existing human-control-guarded IPC handlers. */
  private browsingActions(): Record<'defaultBrowser' | 'library' | 'shortcuts' | 'back' | 'forward', () => unknown> {
    const { bridge, dialog } = this.deps;
    return {
      defaultBrowser: () => bridge.makeDefaultBrowser(),
      library: () => bridge.showLibrary(),
      shortcuts: () => dialog.openShortcuts(),
      back: () => bridge.goBack(),
      forward: () => bridge.goForward(),
    };
  }

  /** The commands that open the workspace panel: Ask (then its box takes focus) and Inspect. */
  private paneActions(): Record<'ask' | 'inspect' | 'network' | 'source', () => unknown> {
    const { panel, host } = this.deps;
    return {
      ask: () => panel.open('chat').then(() => host.focusChat()),
      inspect: () => panel.open('actions'),
      network: () => panel.open('network'),
      source: () => panel.open('source'),
    };
  }

  /** Daily workflows are reachable without opening the web console. */
  private libraryActions(): Record<'playbooks' | 'models' | 'routines' | 'profiles' | 'imports', () => unknown> {
    const { panel, host, dialog } = this.deps;
    return {
      playbooks: () => panel.open('playbooks'),
      models: () => host.openModels?.(),
      routines: () => panel.open('routines'),
      profiles: () => dialog.open(true),
      imports: () => dialog.open(true),
    };
  }

  /** What each forwarded shortcut does. */
  private shortcutActions(): Record<ShortcutName, () => unknown> {
    const { panel, shell, dialog } = this.deps;
    return {
      ...this.actionShortcuts(),
      commands: () => dialog.open(),
      tools: () => panel.toggle(shell.state.recording),
    };
  }

  /** Close the guide or palette before focusing another workspace destination. */
  private actionShortcuts(): Record<CommandId, () => unknown> {
    return Object.fromEntries(Object.keys(this.actions).map((id) => [id, () => this.run(id as CommandId)])) as Record<
      CommandId,
      () => unknown
    >;
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
      const commandsOpen = open && page === 'commands';
      if (commandsOpen && !wasOpen) this.set({ query: '' });
      wasOpen = commandsOpen;
    });
  }
}
