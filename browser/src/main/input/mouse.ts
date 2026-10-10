/** Mouse input through native input along human-like Bézier paths. */
import { nativePointer, nativeWheel, type NativePointerView as PageView } from './native-pointer.ts';
import * as c from './constants.ts';
import { sleep, jitter } from './timing.ts';

/** A point in page pixels. */
export interface Point {
  /** Pixels from the left. */
  x: number;
  /** Pixels from the top. */
  y: number;
}

/** The two inner control points of a cubic Bézier. */
interface Controls {
  /** The first control point. */
  first: Point;
  /** The second control point. */
  second: Point;
}

/** One coordinate of a cubic Bézier at `t`. */
function bezier(t: number, p0: number, p1: number, p2: number, p3: number): number {
  const u = 1 - t;
  const inner = c.CUBIC_INNER_WEIGHT * u * t * (u * p1 + t * p2);
  return u * u * u * p0 + inner + t * t * t * p3;
}

/** Random control points that bend the line from `from` to `to` into a natural arc. */
function controlPoints(from: Point, to: Point, dist: number): Controls {
  const spread = dist * c.PATH_JITTER;
  const stray = (scale: number) => (Math.random() - c.RANDOM_CENTRE) * scale;
  const along = (at: number, scale: number): Point => ({
    x: from.x + (to.x - from.x) * at + stray(spread) * scale,
    y: from.y + (to.y - from.y) * at + stray(spread) * scale,
  });
  return { first: along(c.FIRST_POINT_AT, 1), second: along(c.SECOND_POINT_AT, c.SECOND_POINT_JITTER) };
}

/** How many points a path of `dist` pixels gets. */
const pathSteps = (dist: number): number =>
  Math.max(c.MIN_PATH_STEPS, Math.min(c.MAX_PATH_STEPS, Math.round(dist / c.PX_PER_PATH_STEP)));

/** The point `t` of the way along the curve through `controls`, rounded to whole pixels. */
function pointAt(t: number, from: Point, to: Point, { first, second }: Controls): Point {
  const x = bezier(t, from.x, first.x, second.x, to.x);
  const y = bezier(t, from.y, first.y, second.y, to.y);
  return { x: Math.round(x), y: Math.round(y) };
}

/** The points a hand would pass through from `from` to `to`, fast at the start and slowing near the target. */
export function mousePath(from: Point, to: Point): Point[] {
  const dist = Math.hypot(to.x - from.x, to.y - from.y);
  const steps = pathSteps(dist);
  const controls = controlPoints(from, to, dist);
  return Array.from({ length: steps + 1 }, (_, i) =>
    pointAt(1 - Math.pow(1 - i / steps, c.EASE_POWER), from, to, controls),
  );
}

/**
 * The pointer, moved through native input. It remembers where it was left: only the active
 * tab is driven, so a path always starts where the last one ended.
 */
export class Mouse {
  /** Where the pointer was left. */
  private at: Point = { x: 0, y: 0 };

  /** Move along a curved path; an optional ownership fence runs before every emitted event. */
  async move(view: PageView, x: number, y: number, guard?: () => void): Promise<void> {
    for (const pt of mousePath(this.at, { x, y })) {
      guard?.();
      nativePointer(view, { type: 'mouseMove', x: pt.x, y: pt.y });
      this.at = pt;
      await sleep(jitter(c.MOVE_PAUSE));
    }
    this.at = { x, y };
  }

  /** Moves to the point, hovers, then presses and releases the left button. */
  async click(view: PageView, x: number, y: number): Promise<void> {
    const ix = Math.round(x);
    const iy = Math.round(y);
    await this.move(view, ix, iy);
    await sleep(jitter(c.CLICK_PAUSE)); // a brief hover before the press
    this.button(view, 'mouseDown', { x: ix, y: iy });
    await sleep(jitter(c.CLICK_PAUSE)); // a brief hold before the release
    this.button(view, 'mouseUp', { x: ix, y: iy });
  }

  /** Presses or releases the left button at the native page coordinate. */
  private button(view: PageView, type: 'mouseDown' | 'mouseUp', at: Point): void {
    nativePointer(view, { type, ...at, button: 'left', clickCount: 1 });
  }

  /** One wheel event at (x, y). */
  async scroll(view: PageView, x: number, y: number, deltaX: number, deltaY: number): Promise<void> {
    nativeWheel(view, x, y, deltaX, deltaY);
  }
}
