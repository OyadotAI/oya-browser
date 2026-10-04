/** Mouse input over CDP along human-like Bézier paths. */
import { cdp, type PageView } from '../cdp/cdp.ts';
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
 * The pointer, moved over CDP. It remembers where it was left: only the active
 * tab is driven, so a path always starts where the last one ended.
 */
export class Mouse {
  /** Where the pointer was left. */
  private at: Point = { x: 0, y: 0 };

  /** Moves the pointer to (x, y) along a curved, easing path. */
  async move(view: PageView, x: number, y: number): Promise<void> {
    for (const pt of mousePath(this.at, { x, y })) {
      await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x: pt.x, y: pt.y });
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
    await this.button(view, 'mousePressed', { x: ix, y: iy }, { buttons: 1 });
    await sleep(jitter(c.CLICK_PAUSE)); // a brief hold before the release
    await this.button(view, 'mouseReleased', { x: ix, y: iy });
  }

  /** Presses or releases the left button at `at`; `extra` adds fields after the shared ones. */
  private button(view: PageView, type: string, at: Point, extra: object = {}): Promise<unknown> {
    return cdp(view, 'Input.dispatchMouseEvent', { type, ...at, button: 'left', clickCount: 1, ...extra });
  }

  /** One wheel event at (x, y). */
  async scroll(view: PageView, x: number, y: number, deltaX: number, deltaY: number): Promise<void> {
    await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX, deltaY });
  }
}
