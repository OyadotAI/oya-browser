/**
 * The windows in the veil, as state: each opens over a found element, glides
 * when the element moves, closes when it is gone and opens again if it comes
 * back. A frame asks where each one is and how open, at a given time; the
 * Veil paints the answer. No canvas here, so it is tested on its own.
 */
import { RendererConstants as C } from '../core/constants.ts';
import { between, eased } from './geometry.ts';
import type { Rect } from './types.ts';

/** A box with the id of the element it covers. */
interface Keyed extends Rect {
  /** The element's analysis id. */
  id: number;
}

/** One window, with times from animation frames (null until its first frame since the change). */
interface Hole {
  /** Where its element is. */
  box: Rect;
  /** Where it glides from. */
  from: Rect;
  /** Where it was last drawn, or null before its first frame. */
  drawn: Rect | null;
  /** When it began to open. */
  at: number | null;
  /** When it began its glide. */
  movedAt: number | null;
  /** Whether its element has gone, so it is closing. */
  closing: boolean;
  /** When it began to close. */
  closedAt: number | null;
}

/** One window to paint: where, and how open (0 shut, 1 fully open). */
export interface VeilShape {
  /** Where it is drawn. */
  box: Rect;
  /** How far open it is. */
  open: number;
}

/** One frame of the windows: those to paint, and whether any is still moving. */
export interface VeilFrame {
  /** The windows open at all, in the order they opened. */
  shapes: VeilShape[];
  /** Whether another frame is needed. */
  moving: boolean;
}

/** The veil's windows, by element id. */
export class VeilWindows {
  /** The windows, by element id. */
  private readonly holes = new Map<string, Hole>();

  /** Opens a window over one element; it grows in from its middle over the next frames. */
  open(box: Keyed): void {
    const shut = { drawn: null, at: null, movedAt: null, closing: false, closedAt: null };
    this.holes.set(String(box.id), { box, from: box, ...shut });
  }

  /** The elements moved: each window glides to where its element is, closes when it is gone, and opens again if it is back. */
  move(boxes: readonly Keyed[]): void {
    const at = new Map(boxes.map((box) => [String(box.id), box]));
    for (const [id, hole] of this.holes) follow(hole, at.get(id));
  }

  /** Closes every window at once. */
  clear(): void {
    this.holes.clear();
  }

  /** Where every window is at `now`, and whether any is still opening, gliding or closing. */
  frame(now: number): VeilFrame {
    const frame: VeilFrame = { shapes: [], moving: false };
    for (const hole of this.holes.values()) frame.moving = step(hole, now, frame.shapes) || frame.moving;
    return frame;
  }
}

/** One window follows its element: it glides to where the element is now, closes when it is gone, and reopens if it returns. */
function follow(hole: Hole, box: Rect | undefined): void {
  if (!box) {
    if (!hole.closing) Object.assign(hole, { closing: true, closedAt: null });
    return;
  }
  if (hole.closing) Object.assign(hole, { closing: false, closedAt: null, at: null });
  Object.assign(hole, { from: hole.drawn || hole.box, box, movedAt: null });
}

/** Moves one window on to `now`, adds it to `shapes` while it is open at all, and says whether it is still moving. */
function step(hole: Hole, now: number, shapes: VeilShape[]): boolean {
  stamp(hole, now);
  const glide = eased((now - (hole.movedAt ?? now)) / C.SHIELD_VEIL_GLIDE_MS);
  const open = strength(hole, now);
  hole.drawn = between(hole.from, hole.box, glide);
  if (open > 0) shapes.push({ box: hole.drawn, open });
  return hole.closing ? open > 0 : open < 1 || glide < 1;
}

/** Notes, on a window's first frame since it opened, moved or began to close, when that began. */
function stamp(hole: Hole, now: number): void {
  hole.at ??= now;
  hole.movedAt ??= now;
  if (hole.closing) hole.closedAt ??= now;
}

/** How open a window is: opening eased in, and closing eased out at the same pace once its element has gone. */
function strength(hole: Hole, now: number): number {
  const ms = C.SHIELD_VEIL_OPEN_MS;
  const opened = eased((now - (hole.at ?? now)) / ms);
  return hole.closing ? Math.min(opened, 1 - eased((now - (hole.closedAt ?? now)) / ms)) : opened;
}
