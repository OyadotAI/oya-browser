/**
 * Wheel input on the live view. Deltas are collected and sent as whole-pixel
 * scroll commands, one at a time, so small trackpad deltas add up instead of
 * being lost and a fast flick does not flood the queue.
 */
import { DeltaMode, SCROLL_FLUSH_MS, WHEEL_LINE_PX } from './constants';
import type { LiveIo } from './types';

/** Pixels per delta unit, by `WheelEvent.deltaMode`. Pixel mode (not listed) is 1. */
const DELTA_UNITS: Record<number, (node: HTMLElement) => number> = {
  [DeltaMode.LINE]: () => WHEEL_LINE_PX,
  [DeltaMode.PAGE]: (node) => node.clientHeight,
};

/** Accumulates wheel deltas over one node and sends them as scroll commands. */
export class WheelScroller {
  /** Pixels scrolled and not yet sent; the sign is the direction. */
  private pending = 0;
  /** The page point the wheel was last over. */
  private at = { x: 0, y: 0 };
  /** A scroll command is in flight. */
  private running = false;
  /** The listener was removed: nothing more is sent. */
  private disposed = false;
  /** The next flush, when one is scheduled. */
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** The frame box, whose height is a "page". */
  private readonly node: HTMLElement;
  /** The queue and page mapping. */
  private readonly io: LiveIo;

  /** Scrolls through `io` for wheel events over `node`. */
  constructor(node: HTMLElement, io: LiveIo) {
    this.node = node;
    this.io = io;
  }

  /** The native listener. Watch-only: lets the wheel scroll the dashboard instead of the page. */
  readonly onWheel = (e: WheelEvent) => {
    if (!this.io.interactive || e.ctrlKey || !e.deltaY) return;
    const p = this.io.toPage(e.clientX, e.clientY);
    if (!p) return;
    e.preventDefault();
    e.stopPropagation();
    this.pending += e.deltaY * this.unit(e.deltaMode);
    this.at = { x: p.x, y: p.y };
    if (!this.running && !this.timer) this.schedule();
  };

  /** Stops sending; a scheduled flush is cancelled. */
  dispose() {
    this.disposed = true;
    clearTimeout(this.timer);
  }

  /** Pixels per delta unit for this event's mode. */
  private unit(mode: number): number {
    return Object.hasOwn(DELTA_UNITS, mode) ? DELTA_UNITS[mode](this.node) : 1;
  }

  /** Flushes after a short gather window. */
  private schedule() {
    this.timer = setTimeout(() => void this.flush(), SCROLL_FLUSH_MS);
  }

  /** Sends the whole pixels gathered so far, then goes again if more arrived meanwhile. */
  private async flush() {
    this.timer = undefined;
    if (this.disposed || this.running || Math.abs(this.pending) < 1) return;
    const delta = Math.trunc(this.pending);
    this.pending -= delta;
    this.running = true;
    await this.scroll(delta);
    this.running = false;
    if (!this.disposed && Math.abs(this.pending) >= 1) this.schedule();
  }

  /** One scroll command for `delta` pixels at the last wheel position. */
  private scroll(delta: number) {
    const direction = delta > 0 ? 'down' : 'up';
    const amount = Math.abs(delta);
    this.io.onInput?.(`scroll ${direction} ${amount}`);
    return this.io.enqueue('scroll', { direction, amount, ...this.at, smooth: false, analyze: false });
  }
}

/**
 * Listens for the wheel on `node` and returns the cleanup. React's delegated
 * wheel listener is passive, so preventDefault there cannot stop the
 * dashboard from scrolling underneath the remote page.
 */
export function attachWheel(node: HTMLElement, io: LiveIo): () => void {
  const scroller = new WheelScroller(node, io);
  node.addEventListener('wheel', scroller.onWheel, { passive: false });
  return () => {
    scroller.dispose();
    node.removeEventListener('wheel', scroller.onWheel);
  };
}
