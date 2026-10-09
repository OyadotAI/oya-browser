/** Pure screen-space hit testing for transfers between existing tab strips. */
import type { Point, Rectangle } from 'electron';
import { WINDOW_TAB_STRIP_HEIGHT, DRAG_PREVIEW_OFFSET, DRAG_PREVIEW_SIZE } from './constants.ts';
/** Electron screen and window bounds are both device-independent, including on mixed-DPI monitors. */
export function inTabStrip(point: Point, bounds: Rectangle): boolean {
  return (
    point.x >= bounds.x &&
    point.x < bounds.x + bounds.width &&
    point.y >= bounds.y &&
    point.y < bounds.y + WINDOW_TAB_STRIP_HEIGHT
  );
}

/** Full-window hit testing prevents dropping through a foreground browser into a hidden strip. */
export function inWindow(point: Point, bounds: Rectangle): boolean {
  return (
    point.x >= bounds.x &&
    point.x < bounds.x + bounds.width &&
    point.y >= bounds.y &&
    point.y < bounds.y + bounds.height
  );
}

/** Fit a cursor-offset card inside the current display without covering the pointer at the right edge. */
export function previewPosition(point: Point, area: Rectangle): Point {
  const right = point.x + DRAG_PREVIEW_OFFSET;
  const x =
    right + DRAG_PREVIEW_SIZE.width <= area.x + area.width
      ? right
      : point.x - DRAG_PREVIEW_SIZE.width - DRAG_PREVIEW_OFFSET;
  const y = Math.min(point.y + DRAG_PREVIEW_OFFSET, area.y + area.height - DRAG_PREVIEW_SIZE.height);
  return { x: Math.round(Math.max(area.x, x)), y: Math.round(Math.max(area.y, y)) };
}
