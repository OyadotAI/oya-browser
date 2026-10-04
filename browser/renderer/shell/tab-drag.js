/**
 * Dragging a tab along the strip to reorder it, as in Chrome. A press shows
 * the tab at once; moving a few pixels lifts it, it follows the
 * pointer, its neighbours slide out of its way, and the strip scrolls when
 * the pointer nears an edge. Letting go asks the main process to move it
 * (the order is the main process's); Escape puts it back. Only transforms
 * move during the drag, so nothing is laid out again until it ends.
 */
/* global oyaBrowser, Dom, TabMath, TabCard, TabStrip, RendererConstants */
/* exported TabDrag */

/** The drag in progress, if any. */
const TabDrag = {
  /** `{ item, id, pointerId, startX, startScroll, from, to, slots, items, lifted, x }` while a press lasts, else null. */
  state: null,
  /** The animation frame that scrolls the strip under a dragged tab. */
  frame: 0,

  /** A primary-button press on a tab (not its close button): it is shown now, and may become a drag. */
  press(event, item) {
    if (event.button !== 0 || event.target.closest('.tab-close')) return;
    const id = Number(item.dataset.id);
    oyaBrowser.activateTab(id);
    const start = { startX: event.clientX, x: event.clientX, startScroll: Dom.byId('tab-list').scrollLeft };
    TabDrag.state = { item, id, pointerId: event.pointerId, lifted: false, ...start, ...TabDrag.measure(item) };
    item.setPointerCapture?.(event.pointerId);
  },

  /** Where every tab sits at the press, so the drag moves against a still picture of the strip. */
  measure(item) {
    const items = [...Dom.byId('tab-list').querySelectorAll('.tab-item:not(.closing)')];
    const slots = items.map((node) => ({ left: node.offsetLeft, width: node.offsetWidth }));
    const from = items.indexOf(item);
    return { items, slots, from, to: from };
  },

  /** The pointer moved: past the threshold the tab lifts, then follows. */
  move(event) {
    const state = TabDrag.state;
    if (!state || event.pointerId !== state.pointerId) return;
    state.x = event.clientX;
    if (!state.lifted && Math.abs(state.x - state.startX) < RendererConstants.TAB_DRAG_THRESHOLD) return;
    if (!state.lifted) TabDrag.lift(state);
    TabDrag.follow(state);
  },

  /** The press becomes a drag: the tab rises and the strip starts following the pointer near its edges. */
  lift(state) {
    state.lifted = true;
    TabCard.hide();
    state.item.classList.add('dragging');
    Dom.byId('tab-list').classList.add('dragging');
    TabDrag.frame = requestAnimationFrame(TabDrag.scroll);
  },

  /** Puts the dragged tab under the pointer and slides the others to make room where it would land. */
  follow(state) {
    const list = Dom.byId('tab-list');
    const dx = TabMath.clamp(state.slots, state.from, state.x - state.startX + list.scrollLeft - state.startScroll);
    state.to = TabMath.dropIndex(state.slots, state.from, dx);
    state.item.style.transform = `translateX(${dx}px)`;
    const shifts = TabMath.shifts(state.slots, state.from, state.to);
    state.items.forEach((node, i) => i !== state.from && (node.style.transform = `translateX(${shifts[i]}px)`));
  },

  /** Each frame of a drag: scrolls the strip when the pointer is near its edge, and keeps the tab under it. */
  scroll() {
    const state = TabDrag.state;
    if (!state?.lifted) return;
    const list = Dom.byId('tab-list');
    const rect = list.getBoundingClientRect();
    const speed = TabMath.edgeSpeed(state.x, rect.left, rect.right);
    if (speed) list.scrollLeft += speed;
    if (speed) TabDrag.follow(state);
    TabDrag.frame = requestAnimationFrame(TabDrag.scroll);
  },

  /** The button came up: a drag drops the tab where it is; a plain press was only a click. */
  release(event) {
    const state = TabDrag.state;
    if (!state || event.pointerId !== state.pointerId) return;
    if (state.lifted) TabDrag.drop(state);
    TabDrag.end();
  },

  /** Commits the drop: the strip reorders now, the tab glides from where it was let go, and the main process is told. */
  drop(state) {
    if (state.to === state.from) return;
    const before = state.item.getBoundingClientRect().left;
    const list = Dom.byId('tab-list');
    const others = state.items.filter((node) => node !== state.item);
    list.insertBefore(state.item, others[state.to] || null);
    TabDrag.settle(state.items);
    TabDrag.glide(state.item, before - state.item.getBoundingClientRect().left);
    // Refused (an agent took control): the strip goes back to the main process's order.
    oyaBrowser.moveTab(state.id, state.to).catch(() => TabStrip.update(TabStrip.tabs));
  },

  /** Clears every slide at once, without animating, since the order already puts each tab where it was shown. */
  settle(items) {
    items.forEach((node) => node.classList.add('settling'));
    items.forEach((node) => (node.style.transform = ''));
    void Dom.byId('tab-list').offsetWidth;
    items.forEach((node) => node.classList.remove('settling'));
  },

  /** Starts `item` `offset` pixels off its place and lets it slide home (FLIP). */
  glide(item, offset) {
    if (!offset) return;
    item.classList.add('settling');
    item.style.transform = `translateX(${offset}px)`;
    void item.offsetWidth;
    item.classList.remove('settling');
    item.style.transform = '';
  },

  /** Escape during a drag: every tab slides back to where it started. */
  cancel(event) {
    if (event.key !== 'Escape' || !TabDrag.state?.lifted) return;
    event.preventDefault();
    event.stopPropagation();
    TabDrag.abort();
  },

  /** Drops the drag without moving anything (Escape, or the system took the pointer away). */
  abort() {
    if (!TabDrag.state) return;
    TabDrag.state.items.forEach((node) => (node.style.transform = ''));
    TabDrag.end();
  },

  /** Forgets the press and puts the strip back at rest. */
  end() {
    const state = TabDrag.state;
    TabDrag.state = null;
    cancelAnimationFrame(TabDrag.frame);
    state.item.releasePointerCapture?.(state.pointerId);
    state.item.classList.remove('dragging');
    Dom.byId('tab-list').classList.remove('dragging');
  },
};

document.addEventListener('keydown', TabDrag.cancel, true);
