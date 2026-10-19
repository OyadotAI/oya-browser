/** Screen-space hit testing for transfers between existing tab strips, and where a new window lands. */
import type { Point, Rectangle } from 'electron';
import type { AppServices } from '../app/services.ts';
import { WINDOW_DROP_OFFSET, WINDOW_TAB_STRIP_HEIGHT } from './constants.ts';
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

/** Keep a new window within the target display's work area, including negative-coordinate monitors. */
export function placeNear(scope: AppServices, point: Point): void {
  const area = scope.electron.screen.getDisplayNearestPoint(point).workArea;
  const bounds = scope.shell.window!.getBounds();
  const x = Math.max(area.x, Math.min(point.x - WINDOW_DROP_OFFSET, area.x + area.width - bounds.width));
  const y = Math.max(area.y, Math.min(point.y - WINDOW_DROP_OFFSET, area.y + area.height - bounds.height));
  scope.shell.window!.setPosition(Math.round(x), Math.round(y));
}
