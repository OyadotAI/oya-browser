/**
 * What a tab and the strip do with the pointer and the keyboard: press to
 * show and drag, middle-click to close, right-click for its menu, rest for
 * its hover card; arrows, Home and End to move between tabs, Delete to close
 * the focused one. The handlers measure the DOM and call the ViewModels.
 */
import type { KeyboardEvent, PointerEvent, MouseEvent, RefObject } from 'react';
import type { TabStripViewModel } from '../view-models/tab-strip-view-model.ts';
import type { TabDragViewModel } from '../view-models/tab-drag-view-model.ts';
import type { TabCardViewModel } from '../view-models/tab-card-view-model.ts';
import type { Tab } from '../model/tab-model.ts';
import { measureSlots, stripPort } from '../hooks/use-tab-strip.ts';
import { BUTTONS, CLOSE_KEY } from '../model/constants.ts';
import { groupRange, pinnedCount } from '../model/tab-math.ts';

/** What a tab's handlers act on. */
export interface TabContext {
  /** The tab. */
  tab: Tab;
  /** The strip. */
  strip: TabStripViewModel;
  /** The drag. */
  drag: TabDragViewModel;
  /** The hover card. */
  card: TabCardViewModel;
  /** The tab list element. */
  list: RefObject<HTMLElement | null>;
}

/** A press: the middle button closes on its click (and must not start autoscroll); the primary one may drag. */
function press(ctx: TabContext, event: PointerEvent<HTMLElement>): void {
  ctx.card.hide();
  if (event.button === BUTTONS.middle) return event.preventDefault();
  const onClose = event.target instanceof Element && event.target.closest('.tab-close');
  if (event.button === BUTTONS.primary && !onClose) startDrag(ctx, event);
}

/** A primary press on the tab itself: it shows now and may become a drag, measured against the strip as it stands. */
function startDrag(ctx: TabContext, event: PointerEvent<HTMLElement>): void {
  const list = ctx.list.current;
  if (!list) return;
  const { id, agent } = ctx.tab;
  const press = { id, agent: !!agent, pointerId: event.pointerId, x: event.clientX, y: event.clientY, ...group(ctx) };
  ctx.drag.start({ ...press, ...measureSlots(list, event.currentTarget) }, stripPort(list));
  event.currentTarget.setPointerCapture?.(event.pointerId);
  followPointer(ctx, event.pointerId);
}

/**
 * Hears the press's moves and release on the whole window, not only on the tab. The
 * tab loses pointer capture when a tear-off takes it off this strip, or moves this
 * window under the pointer, and the release then never reached the tab, so the
 * drop was never made. A second release on the tab finds the press already over.
 */
function followPointer(ctx: TabContext, pointerId: number): void {
  const mine = (event: globalThis.PointerEvent) => event.pointerId === pointerId;
  const move = (event: globalThis.PointerEvent) =>
    mine(event) && ctx.drag.move(pointerId, event.clientX, event.clientY);
  const up = (event: globalThis.PointerEvent) =>
    mine(event) && (stopFollowing(move, up), ctx.strip.endPress(pointerId));
  window.addEventListener('pointermove', move, true);
  window.addEventListener('pointerup', up, true);
}

/** The press is over: the window stops passing its moves and release to the strip. */
function stopFollowing(move: (event: globalThis.PointerEvent) => unknown, up: typeof move): void {
  window.removeEventListener('pointermove', move, true);
  window.removeEventListener('pointerup', up, true);
}

/** Where the pressed tab may go: within its own group (pinned or not), and off the strip only when it is not pinned. */
function group({ tab, strip }: TabContext) {
  const tabs = strip.state.tabs;
  return { range: groupRange(pinnedCount(tabs), tabs.length, !!tab.pinned), tearable: !tab.pinned };
}

/** The button came up, or the system took the pointer: the press ends (a drag drops only on `up`). */
function lift(ctx: TabContext, event: PointerEvent<HTMLElement>, up: boolean): void {
  if (up) ctx.strip.endPress(event.pointerId);
  else ctx.drag.end();
  if (event.currentTarget.hasPointerCapture?.(event.pointerId))
    event.currentTarget.releasePointerCapture(event.pointerId);
}

/** The tab's pointer handlers. */
export function pointerHandlers(ctx: TabContext) {
  return {
    onPointerDown: (event: PointerEvent<HTMLElement>) => press(ctx, event),
    onPointerMove: (event: PointerEvent<HTMLElement>) => ctx.drag.move(event.pointerId, event.clientX, event.clientY),
    onPointerUp: (event: PointerEvent<HTMLElement>) => lift(ctx, event, true),
    onPointerCancel: (event: PointerEvent<HTMLElement>) => lift(ctx, event, false),
    onPointerEnter: (event: PointerEvent<HTMLElement>) =>
      ctx.card.hover(ctx.tab, event.currentTarget.getBoundingClientRect().left),
  };
}

/** The tab's click handlers: middle-click closes, right-click asks for the menu, the grow-in ends. */
export function clickHandlers(ctx: TabContext) {
  return {
    onAuxClick: (event: MouseEvent) => event.button === BUTTONS.middle && ctx.strip.closeByMouse(ctx.tab.id),
    onContextMenu: (event: MouseEvent) => (event.preventDefault(), ctx.strip.menu(ctx.tab.id)),
    onAnimationEnd: () => ctx.strip.opened(ctx.tab.id),
  };
}

/** Arrow keys, Home and End move between tabs; Delete closes the focused one. */
export function stripKeydown(event: KeyboardEvent<HTMLElement>, strip: TabStripViewModel): void {
  const target = event.target as HTMLElement;
  const item = target.closest<HTMLElement>('.tab-item');
  if (!target.matches('[role="tab"]') || !item) return;
  if (event.key === CLOSE_KEY) return strip.close(Number(item.dataset.id));
  const next = strip.step(event.key, Number(item.dataset.id));
  if (next === null) return;
  event.preventDefault();
  event.currentTarget.querySelector<HTMLElement>(`.tab-item[data-id="${next}"] [role="tab"]`)?.focus();
}
