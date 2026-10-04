/**
 * Box arithmetic for the shield, and the choices made from it: which found
 * elements are worth outlining, and when the reveal's beam reaches each one.
 * Pure functions, so they are tested without a page.
 */
import { RendererConstants as C } from '../core/constants.ts';
import type { Point, Rect } from './types.ts';

/** The middle of a box. */
export function middle(box: Rect): Point {
  return { x: box.x + box.w * C.SHIELD_HALF, y: box.y + box.h * C.SHIELD_HALF };
}

/** A box grown by `px` on every side. */
export function outset(box: Rect, px: number): Rect {
  return { x: box.x - px, y: box.y - px, w: box.w + px + px, h: box.h + px + px };
}

/** A box scaled by `k` about its middle. */
export function scaled(box: Rect, k: number): Rect {
  const at = middle(box);
  const half = C.SHIELD_HALF;
  return { x: at.x - box.w * k * half, y: at.y - box.h * k * half, w: box.w * k, h: box.h * k };
}

/** Where a box is `t` of the way from `a` to `b`. */
export function between(a: Rect, b: Rect, t: number): Rect {
  const at = (k: keyof Rect): number => a[k] + (b[k] - a[k]) * t;
  return { x: at('x'), y: at('y'), w: at('w'), h: at('h') };
}

/** A fraction of the way through, clamped and eased out (fast, then settling). */
export function eased(t: number): number {
  const clamped = Math.min(1, Math.max(0, t || 0));
  return 1 - (1 - clamped) ** C.SHIELD_VEIL_EASE_POWER;
}

/** A box's area. */
export function area(box: Rect): number {
  return box.w * box.h;
}

/** The area two boxes share. */
export function overlap(a: Rect, b: Rect): number {
  const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
  return Math.max(0, w) * Math.max(0, h);
}

/**
 * Keeps what a person can read on a busy page. Page-sized boxes go, and so does any
 * box that wraps, or mostly repeats, a smaller one already kept: a card around its
 * links, a list around its items. Smallest first, so the innermost controls stay.
 * `page` is the page's area, or 0 when it is not known.
 */
export function declutter<T extends Rect>(boxes: readonly T[], page: number): T[] {
  const kept: T[] = [];
  for (const box of [...boxes].sort((a, b) => area(a) - area(b))) if (keeps(box, kept, page)) kept.push(box);
  return kept;
}

/** Whether `box` is worth an outline beside the ones already `kept`. */
function keeps(box: Rect, kept: readonly Rect[], page: number): boolean {
  if (page && area(box) > page * C.SHIELD_MAX_BOX_SHARE) return false;
  return !kept.some((k) => overlap(box, k) >= area(k) * C.SHIELD_WRAP_SHARE);
}

/** Boxes in reading order: top to bottom, then left to right. */
export function topDown<T extends Rect>(boxes: readonly T[]): T[] {
  return [...boxes].sort((a, b) => a.y - b.y || a.x - b.x);
}

/** When the reveal's beam, crossing a page `height` tall at an even pace, reaches `y`. */
export function beamReaches(y: number, height: number): number {
  const share = Math.min(1, Math.max(0, y) / Math.max(1, height || 0));
  return share * C.SHIELD_REVEAL_MS;
}

/** Whether a box hugs the window's top or left edge, so its number sits just inside its corner rather than off the page. */
export function tucked(box: Rect): boolean {
  const offset = C.SHIELD_TAG_OFFSET_PX;
  return box.x < offset || box.y < offset;
}

/** What Oya says once it has counted `count` elements. */
export function counted(count: number): string {
  return `Found ${count} element${count === 1 ? '' : 's'}`;
}
