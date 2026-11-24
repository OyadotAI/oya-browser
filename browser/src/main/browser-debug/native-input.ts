/** Owned native windows receive genuine mouse events without global injection or DOM activation. */
import { requireDebugScope } from './scope.ts';
import type { DebugObstruction } from './surface.ts';
import { withNativePointerFocus } from '../input/index.ts';
import { createRequire } from 'node:module';
import type { BrowserWindow, Rectangle } from 'electron';
import type { NativePage } from '../native/index.ts';
import type { DebugPoint } from './frames.ts';
/** The composition root supplies this; no native handle comes from MCP or page data. */
export interface NativeMouseTarget {
  /** Exact owning window. */ window: BrowserWindow;
  /** Exact native view bounds. */ bounds: Rectangle;
  /** Verified browser-native obstruction. */ obstruction?: DebugObstruction;
  /** Activate this running application. */ activate?(): void;
}
/** The addon captures bounded focus tokens and dispatches to a proven native first responder. */
interface NativeMouse {
  /** Capture native focus without retaining the old view. */ captureFocus(handle: Buffer): string;
  /** Consume the token; restoration is forbidden after takeover or changed ownership. */ finishFocus(
    token: string,
    restore: boolean,
  ): boolean;
  /** A click pair retains its exact native recipient through release. */ sendMouse(
    handle: Buffer,
    bounds: Rectangle,
    x: number,
    y: number,
    type: string,
  ): string[];
}

let targetOf: ((page: NativePage) => NativeMouseTarget) | undefined;
let native: NativeMouse | undefined;
let addonPath: string | undefined;
/** Native ownership is verified anew before every dispatch. */
export function useNativeMouse(target: NonNullable<typeof targetOf>, file: string): void {
  targetOf = target;
  addonPath = file;
}
/** Load only the packaged application addon; missing capability is an explicit error. */
function debugMouseAdapter(): NativeMouse {
  if (native) return native;
  if (!addonPath) throw Error('Native mouse adapter was not configured');
  try {
    native = createRequire(process.execPath)(addonPath) as NativeMouse;
  } catch {
    throw Error('Native mouse adapter unavailable. Rebuild or update Oya; no DOM fallback was used.');
  }
  return native;
}
/** Entry refusal precedes every page preparation and therefore can promise zero input effects. */
export function requireDebugMouseAvailable(page: NativePage): void {
  if (process.platform !== 'darwin') return;
  const reason = debugMouseWindow(page).obstruction;
  if (!reason) return;
  const text =
    reason === 'browser_control_shield'
      ? 'Native mouse input is unavailable in this macOS Oya configuration while the browser control shield is active. No scrolling, focus change or input occurred. This is a browser control limitation, separate from page DOM overlays. Diagnostics remain available; native interaction cannot currently be verified in this configuration.'
      : 'Native mouse input is unavailable because a browser-native layer prevents verified delivery to this owned page. No scrolling, focus change or input occurred. Diagnostics remain available; native interaction cannot currently be verified in this configuration.';
  throw Object.assign(Error(text), { code: 'native_input_unavailable', reason, dispatched: [] });
}
/** A later native obstruction reports preparation honestly rather than reusing entry's zero-effect copy. */
function requireUnobstructedDebugMouse(target: NativeMouseTarget): void {
  if (target.obstruction)
    throw Error(
      'A browser-native layer now obstructs the owned page after click preparation. No further native input was sent. Inspect before retrying.',
    );
}
const ACTIVATION_WAIT_MS = 1000,
  ACTIVATION_POLL_MS = 20;
/** Window activation may settle asynchronously; no pointer-focus provenance exists while waiting. */
export async function prepareDebugMouse(page: NativePage, guard: () => void, sent: string[]): Promise<void> {
  if (process.platform !== 'darwin') return;
  guard();
  const target = debugMouseWindow(page),
    deadline = Date.now() + ACTIVATION_WAIT_MS;
  requireUnobstructedDebugMouse(target);
  if (target.activate) {
    target.activate();
    sent.push('native_app_focus');
  }
  target.window.focus();
  sent.push('native_window_focus');
  while (!target.window.isFocused()) {
    guard();
    requireDebugScope();
    if (Date.now() >= deadline) throw Error('The owned Oya window did not become active. No pointer input was sent.');
    await new Promise((resolve) => setTimeout(resolve, ACTIVATION_POLL_MS));
  }
  guard();
}
/** A hidden or replaced window is refused instead of revealed or retargeted. */
function debugMouseWindow(page: NativePage): NativeMouseTarget {
  const target = targetOf?.(page);
  if (!target || target.window.isDestroyed() || !target.window.isVisible() || target.window.isMinimized())
    throw Error('Show this lane’s owned Oya window before native input. No click was sent.');
  return target;
}
/** Every native event proves exact WebContents focus and restores permitted prior focus before yielding. */
export function sendDebugMouse(page: NativePage, point: DebugPoint, type: string, guard: () => void): string[] {
  guard();
  const zoom = page.webContents.getZoomFactor();
  if (!Number.isFinite(zoom) || zoom <= 0) throw Error('Native page zoom unavailable');
  if (process.platform !== 'darwin')
    return sendDebugStockMouse(page, { x: point.x * zoom, y: point.y * zoom }, type, guard);
  return withNativePointerFocus(page.webContents, () =>
    dispatchDebugResponder(page, { x: point.x * zoom, y: point.y * zoom }, type, guard),
  );
}
/** One synchronous focus proof owns its native token, dispatch log and restoration decision. */
interface FocusAttempt {
  /** Exact owning native window. */ target: NativeMouseTarget;
  /** Application-only adapter. */ native: NativeMouse;
  /** Verified current window handle. */ handle: Buffer;
  /** One-use prior focus token. */ prior: string;
  /** Completed native events. */ sent: string[];
  /** Failure retained until restoration has finished. */ failure?: unknown;
}
/** Native view focus can never silently select the shield or a personal foreground tab. */
function dispatchDebugResponder(page: NativePage, point: DebugPoint, type: string, guard: () => void): string[] {
  const a = beginDebugFocus(page);
  try {
    a.sent.push('native_focus');
    proveDebugFocus(page, a.target, guard);
    a.sent.push(...a.native.sendMouse(a.handle, a.target.bounds, point.x, point.y, type));
  } catch (why) {
    a.sent.push(...((why as DebugDispatchFailure)?.dispatched ?? []));
    a.failure = why;
  } finally {
    finishDebugFocus(a, page, guard);
  }
  if (a.failure) throw Object.assign(Error(String((a.failure as Error)?.message ?? a.failure)), { dispatched: a.sent });
  return a.sent;
}
/** Capture has no focus side effect and cannot retain another tab's responder. */
function beginDebugFocus(page: NativePage): FocusAttempt {
  const target = debugMouseWindow(page);
  requireUnobstructedDebugMouse(target);
  const native = debugMouseAdapter(),
    handle = target.window.getNativeWindowHandle();
  return { target, native, handle, prior: native.captureFocus(handle), sent: [] };
}
/** Restoration is bounded to the captured window and yields immediately to takeover. */
function finishDebugFocus(a: FocusAttempt, page: NativePage, guard: () => void): void {
  const permitted = mayRestoreDebugFocus(page, a.target, guard);
  try {
    const restored = a.native.finishFocus(a.prior, permitted);
    if (restored) a.sent.push('native_focus_restore');
    else if (permitted) a.failure ??= Error('Prior native focus could not be restored.');
  } catch (why) {
    a.failure ??= why;
  }
}
/** The ownership proof is rechecked after synchronous focus callbacks and immediately before the addon. */
function proveDebugFocus(page: NativePage, target: NativeMouseTarget, guard: () => void): void {
  guard();
  target.window.focus();
  page.webContents.focus();
  if (!page.webContents.isFocused())
    throw Error('Owned-page native focus could not be proven. No pointer input was sent.');
  guard();
  const current = debugMouseWindow(page);
  requireUnobstructedDebugMouse(current);
  if (current.window !== target.window) throw Error('The owned window changed. No pointer input was sent.');
}
/** Takeover, destruction and reparenting supersede restoration rather than being overwritten by it. */
function mayRestoreDebugFocus(page: NativePage, target: NativeMouseTarget, guard: () => void): boolean {
  try {
    guard();
    return debugMouseWindow(page).window === target.window;
  } catch {
    return false;
  }
}
/** Native errors carry the actual completed sequence, including best-effort release. */
interface DebugDispatchFailure {
  /** Native events accepted before failure. */ dispatched?: string[];
}
/** Other platforms use the existing native entry; the trusted exact-frame acknowledgement stays mandatory. */
function sendDebugStockMouse(page: NativePage, point: DebugPoint, type: string, guard: () => void): string[] {
  guard();
  const events = type === 'mouseClick' ? ['mouseDown', 'mouseUp'] : [type],
    sent: string[] = [];
  try {
    for (const event of events) {
      page.webContents.sendInputEvent({ type: event as 'mouseMove', ...point, button: 'left', clickCount: 1 });
      sent.push(event === 'mouseMove' ? 'mousemove' : event === 'mouseDown' ? 'mousedown' : 'mouseup');
    }
  } catch (why) {
    throw Object.assign(why as Error, { dispatched: sent });
  }
  return sent;
}
