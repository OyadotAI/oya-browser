/**
 * Updates. One control does three jobs: it is the version label, the "check
 * now" button, and, once an update is staged, the restart button. The update
 * installs on quit regardless, so nothing here ever blocks browsing.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { TEXT } from '../model/constants.ts';
import { shape, type UpdateStatus } from '../model/models.ts';

/** What the update pill and the version label show. */
export interface UpdatesState {
  /** The pill is hidden: nothing to say. */
  hidden: boolean;
  /** The pill is highlighted: an update is coming. */
  attention: boolean;
  /** The pill cannot be pressed (checking, downloading, restarting). */
  disabled: boolean;
  /** The pill's words. */
  text: string;
  /** The pill's tooltip. */
  title: string;
  /** The version line in the dialog's footer. */
  version: string;
  /** An update is staged: a click restarts. */
  ready: boolean;
  /** The running version ('' while unknown). */
  current: string;
}

/** The parts of the bridge updates use. */
export type UpdatesBridge = Pick<
  OyaBrowser,
  'getUpdateStatus' | 'checkForUpdates' | 'installUpdate' | 'onUpdateStatus'
>;

/** An update status with its running version written for people. */
interface Versioned {
  /** "v1.2.3", or ''. */
  v: string;
}

/** The pill's text for each update state (`v` is "v1.2.3" or ''). */
const LABELS: Record<string, (s: UpdateStatus & Versioned) => string> = {
  checking: () => 'Checking…',
  available: ({ version }) => `Update ${version} available`,
  downloading: ({ version, percent }) => `Downloading ${version}… ${percent ?? 0}%`,
  ready: ({ version }) => `Update ${version} ready, Restart`,
  error: ({ v }) => `${v}, check failed`,
  unsupported: ({ v }) => v,
};

/** States the pill shows itself in. */
const SHOWN = ['available', 'downloading', 'ready', 'error'];

/** The pill's text; any other state means up to date. */
export function updateLabel(status: UpdateStatus): string {
  const v = status.current ? `v${status.current}` : '';
  const state = status.state ?? '';
  return Object.hasOwn(LABELS, state) ? LABELS[state]({ ...status, v }) : `${v}, up to date`;
}

/** The pill's tooltip. */
function updateTitle(state: string, ready: boolean): string {
  if (ready) return 'Restart to finish updating';
  return state === 'unsupported' ? 'Updates apply to installed builds' : 'Click to check for updates';
}

/** Everything the pill and the version line show for `status`. */
export function updatesFor(status: UpdateStatus): UpdatesState {
  const [state, current] = [status.state ?? '', status.current ?? ''];
  const ready = state === 'ready';
  const [text, title] = [updateLabel(status), updateTitle(state, ready)];
  const version = current ? `Oya Browser · v${current}` : 'Oya Browser';
  const attention = ready || state === 'available' || state === 'downloading';
  const disabled = state === 'checking' || state === 'downloading';
  return { hidden: !SHOWN.includes(state), attention, disabled, text, title, version, ready, current };
}

/** The update pill. */
export class UpdatesViewModel extends ViewModel<UpdatesState> {
  /** The main process. */
  private readonly bridge: UpdatesBridge;

  /** Hidden until the updater says something, following it. */
  constructor(bridge: UpdatesBridge) {
    super(updatesFor({}));
    this.bridge = bridge;
    this.own(bridge.onUpdateStatus((status) => this.show(status)));
    bridge.getUpdateStatus().then(
      (status) => this.show(status),
      () => {},
    );
  }

  /** Restarts into a staged update, or checks for one. */
  async click(): Promise<void> {
    if (!this.state.ready) return this.check();
    this.set({ text: TEXT.restarting, disabled: true });
    await this.bridge.installUpdate();
  }

  /** Checks for an update now; a failed check says so. */
  async check(): Promise<void> {
    const current = this.state.current || undefined;
    this.show({ state: 'checking', current });
    const answer = await this.bridge.checkForUpdates().catch(() => ({ state: 'error', current }));
    this.show(answer);
  }

  /** Shows an update status. */
  private show(status: unknown): void {
    this.set(updatesFor(shape<UpdateStatus>(status)));
  }
}
