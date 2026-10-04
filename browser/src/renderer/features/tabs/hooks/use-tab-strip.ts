/**
 * The tab strip's DOM work, next to its views: measuring the room the tabs
 * share, keeping the active tab in view, settling the slides after a drop
 * and gliding the dropped tab home (FLIP), the strip as a drag sees it, and
 * Escape cancelling a drag.
 */
import { useEffect, useLayoutEffect, type RefObject } from 'react';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { Drop, StripPort, TabDragViewModel } from '../view-models/tab-drag-view-model.ts';
import type { TabStripViewModel } from '../view-models/tab-strip-view-model.ts';
import type { Slot } from '../model/tab-math.ts';
import { CANCEL_KEY } from '../model/constants.ts';

/** The id of the commands button, where the tabs' room ends. */
const COMMANDS_ID = 'btn-commands';
/** The selector of the strip's open items. */
const OPEN_ITEMS = '.tab-item:not(.closing)';
/** The class that turns transitions off while slides are cleared. */
const SETTLING = 'settling';

/** The sum of an element's left and right padding. */
function padding(element: Element): number {
  const style = getComputedStyle(element);
  return (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
}

/** The width the tabs may share: from the strip's start to the menu button, less the new-tab button and the drag space. */
function measureRoom(bar: HTMLElement, list: HTMLElement): number {
  const commands = document.getElementById(COMMANDS_ID);
  const end = commands ? commands.getBoundingClientRect().left : bar.getBoundingClientRect().right;
  const newTab = list.nextElementSibling instanceof HTMLElement ? list.nextElementSibling.offsetWidth : 0;
  return end - list.getBoundingClientRect().left - padding(list) - newTab - C.TAB_DRAG_RESERVE;
}

/** An element ref. */
type Ref = RefObject<HTMLElement | null>;

/** Measures the room and tells the strip, once both elements are there. */
function tellRoom(bar: HTMLElement | null, list: HTMLElement | null, vm: TabStripViewModel): void {
  if (bar && list) vm.setRoom(measureRoom(bar, list));
}

/** Tells the strip its room whenever the bar resizes (and when it first shows). */
export function useTabRoom(bar: Ref, list: Ref, vm: TabStripViewModel) {
  useLayoutEffect(() => {
    const observer = new ResizeObserver(() => tellRoom(bar.current, list.current, vm));
    tellRoom(bar.current, list.current, vm);
    if (bar.current) observer.observe(bar.current);
    return () => observer.disconnect();
  }, [bar, list, vm]);
}

/** Scrolls only the strip so `item` is in view, never the page. */
function reveal(list: HTMLElement, item: HTMLElement): void {
  if (item.offsetLeft < list.scrollLeft) list.scrollLeft = item.offsetLeft;
  else if (item.offsetLeft + item.offsetWidth > list.scrollLeft + list.clientWidth) {
    list.scrollLeft = item.offsetLeft + item.offsetWidth - list.clientWidth;
  }
}

/** Keeps the active tab in view after every update. */
export function useRevealActive(list: RefObject<HTMLElement | null>, items: unknown): void {
  useLayoutEffect(() => {
    const active = list.current?.querySelector<HTMLElement>('.tab-item.active');
    if (list.current && active) reveal(list.current, active);
  }, [list, items]);
}

/** Starts `item` `offset` pixels off its place and lets it slide home. */
function glide(item: HTMLElement, offset: number): void {
  if (!offset) return;
  item.classList.add(SETTLING);
  item.style.transform = `translateX(${offset}px)`;
  void item.offsetWidth;
  item.classList.remove(SETTLING);
  item.style.transform = '';
}

/** Clears every slide at once, without animating, since the order already puts each tab where it was shown. */
function settle(list: HTMLElement, items: HTMLElement[]): void {
  items.forEach((node) => node.classList.add(SETTLING));
  void list.offsetWidth;
  items.forEach((node) => node.classList.remove(SETTLING));
}

/** After a drop: the slides clear without animating, then the dropped tab glides home from where it was let go. */
export function useDropSettle(list: RefObject<HTMLElement | null>, drop: Drop | null): void {
  useLayoutEffect(() => {
    if (!drop || !list.current) return;
    const items = [...list.current.querySelectorAll<HTMLElement>(OPEN_ITEMS)];
    settle(list.current, items);
    const item = items.find((node) => node.dataset.id === String(drop.id));
    if (item) glide(item, drop.offset);
  }, [list, drop]);
}

/** Where every open tab sits, and the index of one of them. */
export interface Measured {
  /** Every open tab's slot, in order. */
  slots: Slot[];
  /** The index of the measured tab. */
  index: number;
}

/** Where every open tab sits now, and the index of `item` among them. */
export function measureSlots(list: HTMLElement, item: HTMLElement): Measured {
  const items = [...list.querySelectorAll<HTMLElement>(OPEN_ITEMS)];
  return {
    slots: items.map((node) => ({ left: node.offsetLeft, width: node.offsetWidth })),
    index: items.indexOf(item),
  };
}

/** The strip as a drag sees it. */
export function stripPort(list: HTMLElement): StripPort {
  return {
    scrollLeft: () => list.scrollLeft,
    edges: () => list.getBoundingClientRect(),
    scrollBy: (px) => void (list.scrollLeft += px),
  };
}

/** Escape cancels a lifted drag, and nothing else hears it. */
function cancelOnEscape(event: KeyboardEvent, drag: TabDragViewModel): void {
  if (event.key !== CANCEL_KEY || !drag.cancel()) return;
  event.preventDefault();
  event.stopPropagation();
}

/** Escape during a drag puts the tab back, before anything else hears the key. */
export function useDragCancel(drag: TabDragViewModel): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => cancelOnEscape(event, drag);
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  }, [drag]);
}
