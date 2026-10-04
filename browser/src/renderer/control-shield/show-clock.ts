/**
 * The show's timing, apart from what it draws: when the scan began, when the
 * reveal's pass ends, and which show a timer belongs to, so a timer left by an
 * older show does nothing. The reveal waits for the reading beam to end a
 * sweep, so the beam never jumps back up the page.
 */
import { RendererConstants as C } from '../core/constants.ts';

/** The show's clock. */
export class ShowClock {
  /** Bumped by each new read or dismissal; a timer set under an older one does nothing. */
  private generation = 0;
  /** When the current scan began (ms since the epoch), or null when none is running. */
  private scanStart: number | null = null;
  /** When the reveal's pass reaches the bottom of the page, or null when none is crossing. */
  private passEnds: number | null = null;
  /** The time now, in ms since the epoch. */
  private readonly now: () => number;

  /** `now` reads the time (Date.now by default; tests mock it). */
  constructor(now: () => number = () => Date.now()) {
    this.now = now;
  }

  /** A new read, or the page changed: every timer of the older show is void. */
  bump(): void {
    this.generation += 1;
  }

  /** Runs `fn` after `ms`, unless the show has been bumped by then. */
  later(ms: number, fn: () => void): void {
    const generation = this.generation;
    setTimeout(() => this.runIf(generation, fn), Math.max(0, ms));
  }

  /** Runs `fn` if the show is still the one of `generation`. */
  private runIf(generation: number, fn: () => void): void {
    if (generation === this.generation) fn();
  }

  /** How long a new scan waits for a pass still crossing the page (0 for none); the scan counts from the pass's end. */
  scanWait(): number {
    const wait = this.passEnds === null ? 0 : this.passEnds - this.now();
    if (wait > 0) this.scanStart = this.passEnds;
    return Math.max(0, wait);
  }

  /** The reading beam starts sweeping now. */
  reading(): void {
    this.scanStart = this.now();
    this.passEnds = null;
  }

  /** The reveal's pass starts crossing the page now. */
  revealing(): void {
    this.scanStart = null;
    this.passEnds = this.now() + C.SHIELD_REVEAL_MS;
  }

  /** No scan and no pass: the show let go. */
  stop(): void {
    this.scanStart = null;
    this.passEnds = null;
  }

  /**
   * How long until the reveal may start: the scan's minimum, then the end of the beam's
   * current sweep, so the reveal's pass follows it as the next sweep from the top. No
   * scan running, no wait.
   */
  untilReveal(): number {
    if (this.scanStart === null) return 0;
    const elapsed = this.now() - this.scanStart;
    const atLeast = Math.max(C.SHIELD_MIN_SCAN_MS, elapsed);
    return Math.ceil(atLeast / C.SHIELD_SCAN_LOOP_MS) * C.SHIELD_SCAN_LOOP_MS - elapsed;
  }
}
