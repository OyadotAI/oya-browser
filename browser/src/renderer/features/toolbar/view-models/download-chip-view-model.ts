/**
 * The toolbar's download chip: the Downloads button turns into "Downloading",
 * then "Saved · report.pdf  Open  Show in Finder" when a download from this
 * window ends, so a person sees their click worked. It never takes focus. An
 * ended download stays on the chip while the person can see it (the pointer or
 * focus on it, or the window in the background, holds it), then goes back to
 * the plain button.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { DownloadAction, DownloadBubbleState } from '../../../../shared/ipc.ts';
import { isMac } from '../../../../shared/shortcuts.ts';
import { CHIP_ARM_MS, CHIP_DONE_MS, CHIP_FAILED_MS } from '../model/constants.ts';
import {
  ENDED,
  announcement,
  chipView,
  phaseOf,
  type ChipPhase,
  type DownloadChipView,
} from '../model/download-chip.ts';

/** What the chip shows, and what a screen reader last heard. */
export interface DownloadChipState extends DownloadChipView {
  /** The last announcement, for the polite live region. */
  announce: string;
  /** Whether Show says Finder (macOS) or folder. */
  mac: boolean;
}

/** A download's phase as last drawn. */
interface Seen {
  /** The download. */
  id: string;
  /** Its phase then. */
  phase: ChipPhase;
  /** Whether that phase was announced: a download first seen running waits for the next news. */
  told: boolean;
}

/** What holds an ended download on the chip. */
interface Held {
  /** The pointer is over the chip. */
  pointer: boolean;
  /** Focus is inside the chip. */
  focus: boolean;
}

/** The pending return to the plain button. */
interface Wait {
  /** The timer. */
  handle: ReturnType<typeof setTimeout>;
  /** The download and phase it is for, so news about others does not restart it. */
  key: string;
}

/** The parts of the bridge the chip uses. */
export type DownloadChipBridge = Pick<OyaBrowser, 'onDownloadBubble' | 'downloadAction' | 'showLibrary'>;

/** The download chip. */
export class DownloadChipViewModel extends ViewModel<DownloadChipState> {
  /** The main process. */
  private readonly bridge: DownloadChipBridge;
  /** What main last said about this window's downloads. */
  private bubble: DownloadBubbleState | null = null;
  /** The ended download the person put away, which stays away until another one starts. */
  private dismissed: string | null = null;
  /** The saved download whose file Open or Show found missing. */
  private missing: string | null = null;
  /** The latest download's phase last drawn, so each change is announced once. */
  private seen: Seen | null = null;
  /** Whether the pointer or focus is on the chip: either holds an ended download on it. */
  private held: Held = { pointer: false, focus: false };
  /** The saved file Open and Show may act on: one that has been on screen long enough to be seen. */
  private armed: string | null = null;
  /** The saved file on screen, and the wait before it takes clicks. */
  private arming: { /** The file. */ id: string | null; /** The wait. */ handle?: ReturnType<typeof setTimeout> } = {
    id: null,
  };
  /** The pending return to the plain button, and which download and phase it is for. */
  private timer: Wait | null = null;

  /** The plain button until main tells this window about a download. */
  constructor(bridge: DownloadChipBridge, platform = globalThis.navigator?.platform ?? '') {
    super({ ...chipView(null, 'idle'), announce: '', mac: isMac(platform) });
    this.bridge = bridge;
    this.own(bridge.onDownloadBubble((s) => this.heard(s)));
    this.own(() => (this.stopTimer(), clearTimeout(this.arming.handle)));
  }

  /** The body: opens the Downloads page, and puts an ended download away. */
  showAll(): void {
    void this.bridge.showLibrary('downloads');
    this.putAway();
  }

  /**
   * Open or Show on the file `shown` on screen when clicked: done puts the chip away; a missing file says so
   * instead. Refused when another file has taken the chip since, or this one arrived too recently to have been
   * seen: a page cannot swap its own download in under a click meant for another.
   */
  async act(action: DownloadAction, shown: string): Promise<void> {
    const id = this.state.id;
    if (shown !== id || this.armed !== id) return;
    await this.bridge.downloadAction(id, action).then(
      () => this.putAway(),
      () => ((this.missing = id), this.redraw()),
    );
  }

  /** Escape: puts an ended download away; answers whether it did, so the view can keep focus on the button. */
  escape(): boolean {
    if (!ENDED.has(this.state.phase)) return false;
    this.putAway();
    return true;
  }

  /** The pointer came onto the chip, or left it. */
  pointer(on: boolean): void {
    this.hold('pointer', on);
  }

  /** Focus came into the chip, or left it. */
  focus(on: boolean): void {
    this.hold('focus', on);
  }

  /** Whether focus is in the chip, so the view can keep it there when the part it was on goes away. */
  hasFocus(): boolean {
    return this.held.focus;
  }

  /** Main's news: a new download takes the chip over; the window gaining or losing focus restarts the wait. */
  private heard(s: DownloadBubbleState): void {
    if (s.latest?.id !== this.bubble?.latest?.id) [this.dismissed, this.missing] = [null, null];
    if (s.focused !== this.bubble?.focused) this.stopTimer();
    this.bubble = s;
    this.redraw();
  }

  /** Something holds the chip, or lets it go: either way the wait starts over in full. */
  private hold(what: 'pointer' | 'focus', on: boolean): void {
    this.held[what] = on;
    this.stopTimer();
    this.schedule();
  }

  /** The ended download goes away, back to the plain button (a ring stays while others run). */
  private putAway(): void {
    if (!ENDED.has(this.state.phase)) return;
    this.dismissed = this.state.id;
    this.redraw();
  }

  /** The latest download's phase now. */
  private phase(): ChipPhase {
    const latest = this.bubble?.latest;
    if (!latest || latest.id === this.dismissed) return 'idle';
    return phaseOf(latest, latest.id === this.missing);
  }

  /** Redraws, announces a change of phase, and starts or stops the wait. */
  private redraw(): void {
    const view = chipView(this.bubble, this.phase());
    this.set({ ...view, announce: this.announceFor(view) });
    this.schedule();
    this.arm(view.phase === 'saved' ? view.id : null);
  }

  /** A file newly saved on the chip takes Open and Show only after CHIP_ARM_MS on screen. */
  private arm(id: string | null): void {
    if (id === this.arming.id) return;
    clearTimeout(this.arming.handle);
    this.armed = null;
    this.arming = { id, handle: id ? setTimeout(() => (this.armed = id), CHIP_ARM_MS) : undefined };
  }

  /**
   * What to announce for `view`: each phase once. Every download first arrives running, so "Downloading" waits
   * for news that it still is: one that ends at once is announced only as Saved, never both.
   */
  private announceFor(view: DownloadChipView): string {
    const before = this.seen;
    const same = before?.id === view.id;
    if (view.phase === 'idle' || (same && before!.phase === view.phase && before!.told)) return this.state.announce;
    const told = view.phase !== 'running' || same;
    this.seen = { id: view.id, phase: view.phase, told };
    return told ? announcement(view.phase, this.bubble!.latest!.title) : this.state.announce;
  }

  /** Starts the wait for an ended download nobody is holding, unless it is already running for it. */
  private schedule(): void {
    const { phase, id } = this.state;
    const key = `${id}:${phase}`;
    if (!ENDED.has(phase) || this.isHeld()) return this.stopTimer();
    if (this.timer?.key === key) return;
    this.stopTimer();
    const wait = phase === 'saved' || phase === 'cancelled' ? CHIP_DONE_MS : CHIP_FAILED_MS;
    this.timer = { handle: setTimeout(() => ((this.timer = null), this.putAway()), wait), key };
  }

  /** Whether the person may not be looking: the pointer or focus is on the chip, or the window is behind another. */
  private isHeld(): boolean {
    return this.held.pointer || this.held.focus || !this.bubble?.focused;
  }

  /** Cancels the pending wait. */
  private stopTimer(): void {
    if (this.timer) clearTimeout(this.timer.handle);
    this.timer = null;
  }
}
