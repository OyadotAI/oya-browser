/**
 * The launch: once per app start, Oya wakes up. Its two rings drift together
 * into the mark, light runs around the window's edge, the orb opens its eye,
 * and the stage dissolves into whatever the shell shows. It never holds the
 * person up: a click or a key ends it at once, and with reduced motion it
 * does not play.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { LaunchPhase } from '../model/constants.ts';

/** Where the launch is. */
export interface LaunchState {
  /** Playing, dissolving, or gone. */
  phase: LaunchPhase;
}

/** The launch moment. */
export class LaunchViewModel extends ViewModel<LaunchState> {
  /** The timer that ends the current phase. */
  private timer: ReturnType<typeof setTimeout> | undefined;

  /** Playing from the first paint, timed to dissolve; gone at once with `reducedMotion`. */
  constructor(reducedMotion: boolean) {
    super({ phase: reducedMotion ? 'gone' : 'playing' });
    this.own(() => clearTimeout(this.timer));
    if (!reducedMotion) this.timer = setTimeout(() => this.end(), C.LAUNCH_MS);
  }

  /** Dissolves the stage (on its own, or for a click or a key), then takes it away. */
  end(): void {
    if (this.state.phase !== 'playing') return;
    clearTimeout(this.timer);
    this.set({ phase: 'leaving' });
    this.timer = setTimeout(() => this.set({ phase: 'gone' }), C.LAUNCH_LEAVE_MS);
  }
}
