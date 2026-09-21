/** Mouse input over CDP along human-like Bézier paths. */
const { cdp } = require('../cdp.cjs');
const c = require('./constants.cjs');
const { sleep, jitter } = require('./timing.cjs');

/**
 * Where the pointer was left. One position for every view: only the active
 * tab is driven, so a path always starts where the last one ended.
 */
const pointer = { x: 0, y: 0 };

/** One coordinate of a cubic Bézier at `t`. */
function bezier(t, p0, p1, p2, p3) {
  const u = 1 - t;
  return (
    u * u * u * p0 + c.CUBIC_INNER_WEIGHT * u * u * t * p1 + c.CUBIC_INNER_WEIGHT * u * t * t * p2 + t * t * t * p3
  );
}

/** Random control points that bend the line into a natural arc. */
function controlPoints(x1, y1, x2, y2, dist) {
  const spread = dist * c.PATH_JITTER;
  const stray = (scale) => (Math.random() - c.RANDOM_CENTRE) * scale;
  const cp1x = x1 + (x2 - x1) * c.FIRST_POINT_AT + stray(spread);
  const cp1y = y1 + (y2 - y1) * c.FIRST_POINT_AT + stray(spread);
  const cp2x = x1 + (x2 - x1) * c.SECOND_POINT_AT + stray(spread) * c.SECOND_POINT_JITTER;
  const cp2y = y1 + (y2 - y1) * c.SECOND_POINT_AT + stray(spread) * c.SECOND_POINT_JITTER;
  return { cp1x, cp1y, cp2x, cp2y };
}

/** How many points a path of `dist` pixels gets. */
const pathSteps = (dist) =>
  Math.max(c.MIN_PATH_STEPS, Math.min(c.MAX_PATH_STEPS, Math.round(dist / c.PX_PER_PATH_STEP)));

/** Presses or releases the left button at (x, y); `extra` adds fields after the shared ones. */
const leftButton = (view, type, x, y, extra) =>
  cdp(view, 'Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1, ...extra });

/** The points a hand would pass through from (x1, y1) to (x2, y2). */
function mousePath(x1, y1, x2, y2) {
  const dist = Math.hypot(x2 - x1, y2 - y1);
  const steps = pathSteps(dist);
  const { cp1x, cp1y, cp2x, cp2y } = controlPoints(x1, y1, x2, y2, dist);
  return Array.from({ length: steps + 1 }, (_, i) => {
    // Ease-out: move fast at start, decelerate near target (like a real hand)
    const te = 1 - Math.pow(1 - i / steps, c.EASE_POWER);
    return { x: Math.round(bezier(te, x1, cp1x, cp2x, x2)), y: Math.round(bezier(te, y1, cp1y, cp2y, y2)) };
  });
}

/** Moves the pointer to (toX, toY) along a curved, easing path. */
async function cdpMouseMove(view, toX, toY) {
  const pts = mousePath(pointer.x, pointer.y, toX, toY);
  for (const pt of pts) {
    await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x: pt.x, y: pt.y });
    await sleep(jitter(c.MOVE_PAUSE));
  }
  pointer.x = toX;
  pointer.y = toY;
}

/** Moves to the point, hovers, then presses and releases the left button. */
async function cdpClick(view, x, y) {
  const ix = Math.round(x);
  const iy = Math.round(y);
  await cdpMouseMove(view, ix, iy);
  // Brief hover before click
  await sleep(jitter(c.CLICK_PAUSE));
  await leftButton(view, 'mousePressed', ix, iy, { buttons: 1 });
  // Brief hold before release
  await sleep(jitter(c.CLICK_PAUSE));
  await leftButton(view, 'mouseReleased', ix, iy);
}

/** One wheel event at (x, y). */
async function cdpScroll(view, x, y, deltaX, deltaY) {
  await cdp(view, 'Input.dispatchMouseEvent', { type: 'mouseWheel', x, y, deltaX, deltaY });
}

module.exports = { cdpMouseMove, cdpClick, cdpScroll };
