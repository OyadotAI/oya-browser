/**
 * The Ask pane's profile picker: the project's personas, with the one this
 * browser runs as selected. Choosing another reconnects as it; the server then
 * sends its device and cookies, and the tabs reopen in that persona's own jar.
 * Offline, or while the agent is working, it cannot be changed.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { PersonaList } from '../../../../shared/ipc.ts';
import { ASK_TEXT } from '../model/constants.ts';

/** One choice in the picker. */
export interface PersonaOption {
  /** The persona's id ('default' for the project's default). */
  value: string;
  /** Its name. */
  label: string;
}

/** What the picker shows. */
export interface PersonaState {
  /** The choices: the default first, then the named personas. */
  options: PersonaOption[];
  /** The persona in use. */
  active: string;
  /** The list came from the server, so a choice can be made. */
  ready: boolean;
}

/** The parts of the bridge the picker uses. */
export type PersonaBridge = Pick<OyaBrowser, 'listPersonas' | 'saveConfig' | 'onWsStatus' | 'onFingerprintChanged'>;

/** The id of the project's default persona. */
const DEFAULT_ID = 'default';

/** The default entry, always first. */
const DEFAULT_OPTION: PersonaOption = { value: DEFAULT_ID, label: ASK_TEXT.defaultProfile };

/** What an offline browser lists. */
const OFFLINE: PersonaList = { personas: [], active: DEFAULT_ID };

/** Whether the picker is locked: offline, switching, or while a message is on its way. */
export const personaLocked = (ready: boolean, sending: boolean): boolean => !ready || sending;

/** The profile picker. */
export class PersonaViewModel extends ViewModel<PersonaState> {
  /** The main process. */
  private readonly bridge: PersonaBridge;

  /** Only the default, locked; fills itself now, since a window that loads after the socket connected missed that status. */
  constructor(bridge: PersonaBridge) {
    super({ options: [DEFAULT_OPTION], active: DEFAULT_ID, ready: false });
    this.bridge = bridge;
    this.own(bridge.onWsStatus((status) => this.status(status.connected)));
    this.own(bridge.onFingerprintChanged(() => void this.load()));
    void this.load();
  }

  /** Fills the picker from the server; offline, only the default shows and it is locked. */
  async load(): Promise<void> {
    const { personas, active } = (await this.bridge.listPersonas().catch(() => null)) ?? OFFLINE;
    const named = personas.filter((p) => !p.isDefault).map((p) => ({ value: p.id, label: p.name }));
    this.set({ options: [DEFAULT_OPTION, ...named], active, ready: personas.length > 0 });
  }

  /** Reconnects as the chosen persona, locked until the switch lands. */
  change(value: string): void {
    this.set({ active: value, ready: false });
    void this.bridge.saveConfig({ persona: value });
  }

  /** The connection changed: connected loads the list, offline locks the picker. */
  private status(connected: boolean): void {
    if (connected) void this.load();
    else this.set({ ready: false });
  }
}
