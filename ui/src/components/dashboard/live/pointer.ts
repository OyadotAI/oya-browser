/**
 * Mouse input on the live view: a press grabs the keyboard, a release clicks
 * or drags, a double-click double-clicks, and movement streams when enabled.
 */
import type { MouseEvent } from 'react';
import { DRAG_THRESHOLD_PX, MOUSE_MOVE_INTERVAL_MS, PRIMARY_BUTTON } from './constants';
import type { LiveIo, PagePoint, Ripple, Send } from './types';

/** Where and when the primary button went down. */
export interface Press {
  /** Page x. */
  x: number;
  /** Page y. */
  y: number;
  /** Date.now() at the press. */
  t: number;
}

/** Pointer state that outlives a render: the open press and the last streamed move. */
export interface PointerState {
  /** The press waiting for its release, if any. */
  down: Press | null;
  /** When movement was last streamed. */
  lastMove: number;
}

/** What the pointer handlers need from the live view. */
export interface PointerContext extends LiveIo {
  /** Moves are streamed only while this is on. */
  hover: boolean;
  /** Sends what was typed before a click lands. */
  flush: () => void;
  /** Focuses the frame and takes the keyboard. */
  grab: () => void;
  /** Draws the click ripple. */
  setRipple: (ripple: Ripple) => void;
  /** Unqueued send, for mouse moves that may be dropped. */
  send: Send;
  /** Survives renders. */
  state: PointerState;
}

/** Primary button down on the frame: take the keyboard and remember where. */
export function mouseDown(e: MouseEvent, ctx: PointerContext) {
  if (e.button !== PRIMARY_BUTTON || !ctx.interactive) return;
  const p = ctx.toPage(e.clientX, e.clientY);
  if (!p) return;
  e.preventDefault();
  ctx.flush();
  ctx.grab();
  ctx.state.down = { x: p.x, y: p.y, t: Date.now() };
}

/** A press that travelled: one drag from where it started. */
function drag(start: Press, p: PagePoint, ctx: PointerContext) {
  ctx.onInput?.(`drag ${start.x},${start.y} → ${p.x},${p.y}`);
  void ctx.enqueue('drag', { from_x: start.x, from_y: start.y, to_x: p.x, to_y: p.y });
}

/** A press that stayed put: a click, with a ripple where it landed. */
function click(p: PagePoint, ctx: PointerContext) {
  ctx.setRipple({ x: p.localX, y: p.localY, id: Date.now() });
  ctx.onInput?.(`click ${p.x},${p.y}`);
  void ctx.enqueue('click_coordinates', { x: p.x, y: p.y });
}

/** Button up: a click, or a drag when it moved past the threshold. */
export function mouseUp(e: MouseEvent, ctx: PointerContext) {
  const start = ctx.state.down;
  ctx.state.down = null;
  const p = ctx.toPage(e.clientX, e.clientY);
  if (!p || !start) return;
  if (Math.hypot(p.x - start.x, p.y - start.y) > DRAG_THRESHOLD_PX) return drag(start, p, ctx);
  click(p, ctx);
}

/** A double-click on the page. */
export function doubleClick(e: MouseEvent, ctx: PointerContext) {
  if (!ctx.interactive) return;
  const p = ctx.toPage(e.clientX, e.clientY);
  if (!p) return;
  ctx.onInput?.(`double-click ${p.x},${p.y}`);
  void ctx.enqueue('double_click', { x: p.x, y: p.y });
}

/** Streams the pointer while hover is on, throttled, and outside the queue. */
export function mouseMove(e: MouseEvent, ctx: PointerContext) {
  if (!ctx.hover || !ctx.interactive) return;
  const now = Date.now();
  if (now - ctx.state.lastMove < MOUSE_MOVE_INTERVAL_MS) return;
  ctx.state.lastMove = now;
  const p = ctx.toPage(e.clientX, e.clientY);
  if (p) void ctx.send('mouse_move', { x: p.x, y: p.y });
}
