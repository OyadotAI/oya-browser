/**
 * Sync on the account page: when this browser's logins last reached the
 * server, how many sites it keeps, and Sync now, whose confirmation arrives
 * separately (profile-saved) or never, in which case the button comes back.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus, ShellStatus } from '../../../../shared/ipc.ts';
import { TEXT } from '../model/constants.ts';
import { messageOf, shape, type Config, type ProfileSaved } from '../model/models.ts';

/** What the sync line knows. */
export interface SyncState {
  /** When the logins last went to the server (epoch milliseconds, 0 for never). */
  at: number;
  /** Sites the server said it keeps at the last Sync now, or null before one. */
  sites: number | null;
  /** Whether the control socket is connected. */
  connected: boolean;
  /** A Sync now waits for its confirmation. */
  pending: boolean;
  /** The outcome under the row ('' for none). */
  note: string;
}

/** The sync line: a lead, the time it names, and the count; or, before any sync, what has to happen. */
export interface SyncLine {
  /** "Logins synced " or "Last synced ", or the whole line when `at` is 0. */
  lead: string;
  /** When, or 0 for no time. */
  at: number;
  /** " · 937 sites", or ''. */
  count: string;
}

/** The parts of the bridge sync uses. */
export type SyncBridge = Pick<OyaBrowser, 'saveProfile' | 'onProfileSaved' | 'onWsStatus'>;

/** The sync line for `state`. */
export function syncLine({ at, sites, connected }: SyncState): SyncLine {
  if (!at) {
    const lead = connected ? 'Logins sync while you are connected' : 'Logins sync when you reconnect';
    return { lead, at: 0, count: '' };
  }
  const count = sites === null ? '' : ` · ${sites} site${sites === 1 ? '' : 's'}`;
  return { lead: connected ? 'Logins synced ' : 'Last synced ', at, count };
}

/** Whether Sync now can be pressed. */
export const canSync = (state: SyncState): boolean => state.connected && !state.pending;

/** Sync on the account page. */
export class SyncViewModel extends ViewModel<SyncState> {
  /** The main process. */
  private readonly bridge: SyncBridge;
  /** Gives the button back if the sync is never confirmed. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** Never synced, offline, following the connection and the server's confirmations. */
  constructor(bridge: SyncBridge) {
    super({ at: 0, sites: null, connected: false, pending: false, note: '' });
    this.bridge = bridge;
    this.own(() => clearTimeout(this.timer));
    this.own(bridge.onWsStatus((status) => this.onStatus(status)));
    this.own(bridge.onProfileSaved((answer) => this.saved(shape<ProfileSaved>(answer))));
  }

  /** Takes the saved sync record and the live status: the later of the last Sync now and the last automatic send. */
  load(config: Config, status: Partial<ShellStatus>): void {
    const at = Math.max(config.lastSync?.at || 0, status.syncedAt || 0);
    this.set({ at, sites: config.lastSync ? config.lastSync.sites : null, connected: !!status.connected });
  }

  /** The connection came or went. */
  onStatus(status: ConnectionStatus): void {
    this.set({ connected: !!status.connected });
  }

  /** Sends every login to the server now; the confirmation arrives separately. */
  async save(): Promise<void> {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.finish(TEXT.syncUnconfirmed), C.PROFILE_SAVE_TIMEOUT_MS);
    this.set({ note: TEXT.syncing, pending: true });
    await this.bridge.saveProfile().catch((error: unknown) => this.finish(messageOf(error)));
  }

  /** The server confirmed a sync: the line now says just now, and how many sites it keeps. */
  private saved(answer: ProfileSaved): void {
    if (!answer.error) this.set({ at: Date.now(), sites: answer.sites?.length || 0 });
    this.finish(answer.error || '');
  }

  /** Shows the outcome and gives the button back. */
  private finish(note: string): void {
    clearTimeout(this.timer);
    this.set({ note, pending: false });
  }
}
