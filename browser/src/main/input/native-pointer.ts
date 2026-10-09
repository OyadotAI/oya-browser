/** Native page input: no debugger attachment, protocol transport, or CDP fallback. */
import type { MouseInputEvent, MouseWheelInputEvent, WebContents } from 'electron';
/** Minimal native capability; callers need neither a debugger nor an automation connection. */
export interface NativePointerView {
  /** Existing page contents receive input through Electron's native event path. */
  webContents: Pick<WebContents, 'isDestroyed' | 'sendInputEvent'>;
}
/** Reject invalid coordinates before they enter the native event system. */
export function nativePointer(view: NativePointerView, event: MouseInputEvent | MouseWheelInputEvent): void {
  if (view.webContents.isDestroyed()) throw new Error('View is destroyed');
  if (![event.x, event.y].every(Number.isFinite)) throw new Error('Pointer coordinates must be finite');
  view.webContents.sendInputEvent({ ...event, x: Math.round(event.x), y: Math.round(event.y) });
}
/** Browser actions express positive deltas as right/down; native wheel deltas use the opposite sign. */
export function nativeWheel(view: NativePointerView, x: number, y: number, deltaX: number, deltaY: number): void {
  if (![deltaX, deltaY].every(Number.isFinite)) throw new Error('Wheel deltas must be finite');
  nativePointer(view, { type: 'mouseWheel', x, y, deltaX: -deltaX || 0, deltaY: -deltaY || 0 });
}
