/** Native pointer and keyboard actions consume exact isolated node capabilities. */
import type { PageDriver } from '../actions/driver.ts';
import { actionNavigation } from '../actions/action-navigation.ts';
import { nativePointer } from '../input/index.ts';
import type { NativeSelection } from './native-target-operations.ts';
import type { NativePoint } from './native-scope.ts';
import { targetOperation, targetPoint } from './native-target-operations.ts';
import { NATIVE_VALIDATION } from './constants.ts';
/** Input dispatch remains owned by the admitted run across every asynchronous native boundary. */
export interface ElementInput {
  /** Production native devices. */ driver: PageDriver;
  /** Exact selected node. */ target: NativeSelection;
  /** Stop/tab ownership check. */ guard(): void;
}
/** Move to the exact target, then recheck geometry before a trusted button event. */
async function readyPointer(input: ElementInput): Promise<{ /** Point x. */ x: number; /** Point y. */ y: number }> {
  const point = await paintedPoint(input);
  await input.driver.mouse.move(input.target.scope.tab.view, point.x, point.y, input.guard);
  input.guard();
  const current = await targetPoint(input.target);
  input.guard();
  unchangedPoint(point, current);
  await awaitPointer(input, current);
  return current;
}
/** Trusted native movement must reach the exact child node before any button event is admitted. */
async function awaitPointer(input: ElementInput, point: NativePoint) {
  const deadline = Date.now() + NATIVE_VALIDATION.POINTER_MS;
  while (!(await pointerReady(input, point, deadline)))
    await new Promise((resolve) => setTimeout(resolve, NATIVE_VALIDATION.POLL_MS));
  input.guard();
}
/** Retry movement at the same verified point while nested compositor routing settles. */
async function pointerReady(input: ElementInput, point: NativePoint, deadline: number) {
  input.guard();
  unchangedPoint(point, await targetPoint(input.target));
  input.guard();
  if (await targetOperation<boolean>(input.target, 'pointerReached')) return true;
  input.guard();
  if (Date.now() >= deadline) throw Error('Workflow native pointer did not reach the target');
  nativePointer(input.target.scope.tab.view, { type: 'mouseMove', ...point });
  return false;
}
/** Scrolled cross-process frames must publish compositor hit-test data before native input. */
async function paintedPoint(input: ElementInput) {
  input.guard();
  const point = await targetPoint(input.target, true);
  input.guard();
  await input.target.scope.tab.view.webContents.capturePage();
  input.guard();
  await targetOperation(input.target, 'armPointer');
  input.guard();
  return point;
}
/** Native click counts preserve browser double-click behavior without synthetic DOM events. */
export async function clickElement(input: ElementInput, double = false): Promise<void> {
  await actionNavigation(input.target.scope.tab.view, async () => {
    const point = await readyPointer(input);
    for (let count = 1; count <= (double ? NATIVE_VALIDATION.DOUBLE_CLICK : 1); count++) {
      input.guard();
      nativePointer(input.target.scope.tab.view, { type: 'mouseDown', ...point, button: 'left', clickCount: count });
      nativePointer(input.target.scope.tab.view, { type: 'mouseUp', ...point, button: 'left', clickCount: count });
    }
  });
}
/** Hover uses the native mouse path and rejects covered target owners. */
export async function hoverElement(input: ElementInput): Promise<void> {
  const point = await targetPoint(input.target, true);
  input.guard();
  await input.driver.mouse.move(input.target.scope.tab.view, point.x, point.y, input.guard);
  input.guard();
  await targetOperation(input.target, 'valid');
}
/** Ordinary fields use native editing; browser-specific date controls use their explicit DOM value contract. */
export async function typeElement(input: ElementInput, text: string): Promise<void> {
  const info = await targetOperation<{ /** Native input type. */ type: string }>(input.target, 'editable');
  input.guard();
  if (NATIVE_VALIDATION.VALUE_INPUTS.some((type) => type === info.type)) return setSpecialValue(input, text);
  await clickElement(input);
  await awaitFocus(input);
  await input.driver.keyboard.clear(input.target.scope.tab.view);
  await typeCharacters(input, text);
  await focused(input);
}
/** Focus is checked in the exact child document after every awaited native edit. */
async function focused(input: ElementInput): Promise<void> {
  input.guard();
  await targetOperation(input.target, 'focused');
  input.guard();
}
/** Explicit special-input assignment follows the same input/change contract as standalone workflow fill. */
async function setSpecialValue(input: ElementInput, text: string): Promise<void> {
  input.guard();
  await targetOperation(input.target, 'fillValue', { value: text });
  input.guard();
}
/** Native isolated DOM option selection preserves label matching, form events and actual selected value. */
export async function selectElement(input: ElementInput, label: string): Promise<void> {
  input.guard();
  await targetOperation(input.target, 'select', { value: label });
  input.guard();
}

/** A moved target is never chased after the native pointer has reached a different point. */
function unchangedPoint(
  before: { /** Point x. */ x: number; /** Point y. */ y: number },
  after: { /** Point x. */ x: number; /** Point y. */ y: number },
): void {
  const moved =
    Math.abs(before.x - after.x) > NATIVE_VALIDATION.POINT_TOLERANCE ||
    Math.abs(before.y - after.y) > NATIVE_VALIDATION.POINT_TOLERANCE;
  if (moved) throw Error('Workflow target moved before input');
}
/** Stop and focus loss interrupt long typing runs before another character is emitted. */
async function typeCharacters(input: ElementInput, text: string): Promise<void> {
  for (const char of text) {
    await focused(input);
    await input.driver.keyboard.type(input.target.scope.tab.view, char);
  }
}

/** Cross-process native input acknowledgement may follow an isolated evaluation reply. */
async function awaitFocus(input: ElementInput): Promise<void> {
  const deadline = Date.now() + NATIVE_VALIDATION.FOCUS_MS;
  while (!(await focusReady(input, deadline)))
    await new Promise((resolve) => setTimeout(resolve, NATIVE_VALIDATION.POLL_MS));
}
/** Retry only pending native focus, never detached nodes or changed control. */
async function focusReady(input: ElementInput, deadline: number): Promise<boolean> {
  try {
    await focused(input);
    return true;
  } catch (error) {
    if (!String(error).includes('Workflow input focus changed') || Date.now() >= deadline) throw error;
    return false;
  }
}
