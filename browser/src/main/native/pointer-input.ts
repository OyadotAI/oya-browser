/** External pointer semantics translated to browser-owned input, never an engine debugging protocol. */
import type { MouseInputEvent, MouseWheelInputEvent } from 'electron';
import type { NativePage } from './page.ts';
import { nativePointerExact } from '../input/index.ts';
import { INPUT_LIMITS, INPUT_MODIFIERS, INPUT_BUTTONS, BUTTON_MODIFIERS } from './constants.ts';
/** Protocol-independent operation names accepted by the native boundary. */
const TYPES = {
  mousePressed: 'mouseDown',
  mouseReleased: 'mouseUp',
  mouseMoved: 'mouseMove',
  mouseWheel: 'mouseWheel',
} as const;
/** Typed pointer request after boundary validation. */
type Params = Record<string, unknown>;
/** Bounded integral bitmasks prevent accidental coercion of flags. */
export function inputBits(value: unknown, max: number): number {
  if (value === undefined) return 0;
  if (!Number.isInteger(value) || Number(value) < 0 || Number(value) > max) throw Error('Invalid native input flags');
  return Number(value);
}
/** Standard modifiers, without a grant to browser shortcuts or another renderer. */
export function inputModifiers(value: unknown): NonNullable<MouseInputEvent['modifiers']> {
  const bits = inputBits(value, INPUT_LIMITS.modifiers);
  return INPUT_MODIFIERS.filter(([bit]) => bits & bit).map(([, name]) => name);
}
/** Pointer coordinates and deltas must be real bounded CSS pixels. */
function coordinate(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || Math.abs(value) > INPUT_LIMITS.coordinate)
    throw Error('Invalid native pointer coordinate or delta');
  return value;
}
/** Validate every semantic field before sending the first native event. */
export function validatePointer(params: Params): void {
  if (typeof params.type !== 'string' || !Object.hasOwn(TYPES, params.type))
    throw Error('Unsupported native pointer type');
  coordinate(params.x);
  coordinate(params.y);
  inputModifiers(params.modifiers);
  buttonState(params);
  inputBits(params.clickCount, INPUT_LIMITS.clicks);
  validateWheel(params);
}
/** Wheel semantics are never accepted on ordinary mouse events. */
function validateWheel(params: Params): void {
  if (params.type === 'mouseWheel') {
    if (params.clickCount !== undefined && params.clickCount !== 0) throw Error('Wheel events cannot click');
    coordinate(params.deltaX);
    coordinate(params.deltaY);
  } else if (params.deltaX !== undefined || params.deltaY !== undefined)
    throw Error('Wheel deltas require a wheel event');
}
/** Reject unsupported buttons and contradictory held-button masks. */
function buttonState(params: Params): number {
  const bit = buttonBit(params);
  const inferred = params.type === 'mousePressed' ? bit : 0;
  const held = params.buttons === undefined ? inferred : inputBits(params.buttons, INPUT_LIMITS.buttons);
  if ((params.type === 'mousePressed' && !(held & bit)) || (params.type === 'mouseReleased' && held & bit))
    throw Error('Native mouse button state contradicts the event');
  return held;
}
/** Physical press/release events must identify a supported button. */
function buttonBit(params: Params): number {
  const button = params.button === undefined ? 'none' : params.button;
  if (typeof button !== 'string' || (button !== 'none' && !Object.hasOwn(INPUT_BUTTONS, button)))
    throw Error('Unsupported native mouse button');
  const bit = INPUT_BUTTONS[button as keyof typeof INPUT_BUTTONS] ?? 0;
  if (['mousePressed', 'mouseReleased'].includes(String(params.type)) && !bit)
    throw Error('A pressed or released button is required');
  return bit;
}
/** Convert CSS pixels using the exact page's zoom, not the focused window or device scale. */
export function dispatchNativePointer(page: NativePage, params: Params): object {
  validatePointer(params);
  if (page.webContents.isDestroyed()) throw Error('View is destroyed');
  const zoom = page.webContents.getZoomFactor();
  if (!Number.isFinite(zoom) || zoom <= 0) throw Error('Native page zoom is unavailable');
  nativePointerExact(page, pointerEvent(params, zoom));
  return {};
}
/** Native move/wheel events use modifiers for held buttons; no-button does not invent a press. */
function pointerEvent(params: Params, zoom: number): MouseInputEvent | MouseWheelInputEvent {
  const event = pointerBase(params, zoom);
  if (params.button !== undefined && params.button !== 'none')
    event.button = params.button as MouseInputEvent['button'];
  if (params.type !== 'mouseWheel') return { ...event, clickCount: Number(params.clickCount ?? 0) };
  return wheelEvent(event, params, zoom);
}
/** Pointer metadata is derived from validated CSS coordinates and explicit held-state flags. */
function pointerBase(params: Params, zoom: number): MouseInputEvent {
  const held = buttonState(params);
  const buttons = BUTTON_MODIFIERS.filter(([bit]) => held & bit).map(([, name]) => name);
  return {
    type: TYPES[params.type as keyof typeof TYPES],
    x: coordinate(params.x) * zoom,
    y: coordinate(params.y) * zoom,
    modifiers: [...inputModifiers(params.modifiers), ...buttons],
  };
}
/** DOM wheel direction is the inverse of native wheel deltas. */
function wheelEvent(event: MouseInputEvent, params: Params, zoom: number): MouseWheelInputEvent {
  return {
    ...event,
    type: 'mouseWheel',
    deltaX: -coordinate(params.deltaX) * zoom,
    deltaY: -coordinate(params.deltaY) * zoom,
    hasPreciseScrollingDeltas: true,
  };
}
