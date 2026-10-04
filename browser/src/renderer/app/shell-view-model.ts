/**
 * The shell's own state, which several features read: whether the browser is
 * connected, whether the window shows the welcome screen or browsing, whether
 * the start page is up, and whether a workflow is being recorded.
 */
import { ViewModel } from '../core/view-model.ts';
import type { OyaBrowser } from '../core/bridge.ts';

/** The shell's mode: the welcome screen, or the browser. */
export type ShellMode = 'setup' | 'browsing';

/** What the shell shows. */
export interface ShellState {
  /** Connected to the server. */
  connected: boolean;
  /** The welcome screen or the browser. */
  mode: ShellMode;
  /** The active tab is the start page. */
  onHome: boolean;
  /** A workflow is being recorded. */
  recording: boolean;
}

/** The shell's state, fed by the main process and by the features that own each part. */
export class ShellViewModel extends ViewModel<ShellState> {
  /** Starts on the welcome screen, offline, and follows the main process's mode and connection. */
  constructor(bridge: Pick<OyaBrowser, 'onModeChanged' | 'onWsStatus'>) {
    super({ connected: false, mode: 'setup', onHome: false, recording: false });
    this.own(bridge.onModeChanged((mode) => this.set({ mode })));
    this.own(bridge.onWsStatus((status) => this.set({ connected: !!status.connected })));
  }

  /** The connection changed (the connection feature also sets it from its first status read). */
  setConnected(connected: boolean): void {
    this.set({ connected });
  }

  /** The shell left or entered the welcome screen. */
  setMode(mode: ShellMode): void {
    this.set({ mode });
  }

  /** The active tab is, or stopped being, the start page. */
  setOnHome(onHome: boolean): void {
    this.set({ onHome });
  }

  /** A recording started or ended. */
  setRecording(recording: boolean): void {
    this.set({ recording });
  }
}
