/** Native clicks require exact document/node capabilities and trusted pointer acknowledgement. */
import type { PageDriver } from '../actions/driver.ts';
import type { DebugElement, DebugRegistry } from './registry.ts';
import { topPoint, type DebugPoint } from './frames.ts';
import { sendDebugMouse, prepareDebugMouse, requireDebugMouseAvailable } from './native-input.ts';
import { requireDebugScope } from './scope.ts';
import { withinTime } from '../../shared/within-time.ts';
const POINTER_WAIT_MS = 1000,
  POINTER_POLL_MS = 20,
  POINT_TOLERANCE = 0.5;
/** One attempt owns its exact native target, guard and dispatch log until cleanup. */
interface Attempt {
  /** Native registry. */ registry: DebugRegistry;
  /** Exact node. */ target: DebugElement;
  /** Current admission. */ guard(): void;
  /** Completed events. */ sent: string[];
}
/** Completed native events are reported independently of page activation. */
export async function clickDebugElement(driver: PageDriver, registry: DebugRegistry, target: DebugElement) {
  const attempt: Attempt = { registry, target, sent: [], guard: () => requireOwnedDebugView(driver, registry) };
  attempt.guard();
  requireDebugMouseAvailable(registry.page);
  try {
    return await performDebugClick(attempt);
  } catch (why) {
    throw debugClickFailure(why, attempt);
  } finally {
    await registry.run(target.document, 'release_pointer').catch(() => {});
  }
}
/** Movement, hover revalidation and buttons form one bounded native attempt. */
async function performDebugClick(a: Attempt) {
  const point = await debugTargetPoint(a, true);
  await prepareDebugPointer(a);
  dispatchDebugMouse(a, point, 'mouseMove', 'mousemove');
  await awaitDebugPointer(a, point);
  requireDebugUnmoved(point, await debugTargetPoint(a, false));
  await debugClickButtons(a, point);
  return { input: 'native_mouse', point, frame: a.target.document.ref, dispatched: a.sent, outcome: 'dispatched' };
}
/** Rendering and native activation settle before any movement or button input. */
async function prepareDebugPointer(a: Attempt): Promise<void> {
  await withinTime(
    a.registry.page.webContents.capturePage(),
    POINTER_WAIT_MS,
    'Native compositor did not become ready. No input was sent.',
  );
  await a.registry.run(a.target.document, 'arm_pointer', { element: a.target.local });
  await prepareDebugMouse(a.registry.page, a.guard, a.sent);
}
/** A completed down always gets a best-effort up on the same owned view, never another tab. */
async function debugClickButtons(a: Attempt, point: DebugPoint): Promise<void> {
  a.guard();
  await a.registry.run(a.target.document, 'metadata');
  dispatchDebugMouse(a, point, 'mouseClick', 'mousedown/mouseup');
}
/** Record only events the native adapter accepted. */
function dispatchDebugMouse(a: Attempt, point: DebugPoint, type: string, _name: string): void {
  a.sent.push(...sendDebugMouse(a.registry.page, point, type, a.guard));
}
/** Admission cannot turn into input in a different lane, tab, profile or control state. */
function requireOwnedDebugView(driver: PageDriver, registry: DebugRegistry): void {
  requireDebugScope();
  if (registry.page.webContents.isDestroyed() || driver.deps.getActiveView() !== registry.page)
    throw Error('The owned browser tab changed. No further input was sent.');
  driver.deps.diagnosticGuard?.(registry.page);
}
/** Scrolling precedes the actual client-rect sample and verified parent projection. */
async function debugTargetPoint(a: Attempt, scroll: boolean): Promise<DebugPoint> {
  const local = await a.registry.run<DebugPoint>(a.target.document, 'point', { element: a.target.local, scroll });
  return topPoint(a.registry, a.target.document, local, scroll);
}
/** Trusted movement must reach the intended document before a button press is attempted. */
async function awaitDebugPointer(a: Attempt, point: DebugPoint): Promise<void> {
  const deadline = Date.now() + POINTER_WAIT_MS;
  while (!(await a.registry.run<boolean>(a.target.document, 'pointer_reached'))) {
    a.guard();
    if (Date.now() >= deadline)
      throw Error('Native pointer did not reach the intended frame. No button press was sent.');
    await new Promise((resolve) => setTimeout(resolve, POINTER_POLL_MS));
    dispatchDebugMouse(a, point, 'mouseMove', 'mousemove');
  }
}
/** Hover-induced motion is refused instead of chased into a different action point. */
function requireDebugUnmoved(before: DebugPoint, after: DebugPoint): void {
  if (Math.abs(before.x - after.x) > POINT_TOLERANCE || Math.abs(before.y - after.y) > POINT_TOLERANCE)
    throw Error('Element moved before input. No button press was sent.');
}
/** Partial input is explicit and is never automatically replayed. */
function debugClickFailure(why: unknown, a: Attempt): Error {
  a.sent.push(...((why as { /** Native partial sequence. */ dispatched?: string[] })?.dispatched ?? []));
  const message = String((why as Error)?.message ?? why);
  return Object.assign(
    Error(`${message} Completed native events: ${a.sent.join(', ') || 'none'}. Inspect before retrying.`),
    { dispatched: a.sent },
  );
}
