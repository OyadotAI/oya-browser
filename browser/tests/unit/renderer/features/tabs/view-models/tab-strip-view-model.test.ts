/**
 * Unit tests for the tab strip: it keeps the main process's order, folds
 * closed tabs away, holds widths after a close with the mouse, answers the
 * keyboard, middle-click and right-click, and reorders on a drop (and back
 * when the main process refuses).
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  TabStripViewModel,
  stripLayout,
} from '../../../../../../src/renderer/features/tabs/view-models/tab-strip-view-model.ts';
import { TabDragViewModel } from '../../../../../../src/renderer/features/tabs/view-models/tab-drag-view-model.ts';
import { TabCardViewModel } from '../../../../../../src/renderer/features/tabs/view-models/tab-card-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeBridge, manualFrames } from '../../../support/bridge.ts';

/** A tab as the main process summarizes it. */
const tab = (id: number, extra = {}) => ({
  id,
  title: `Tab ${id}`,
  url: `https://${id}.test/`,
  home: false,
  favicon: null,
  active: false,
  ...extra,
});

/** A strip over a fake bridge, with its drag and card. */
function strip({ reduced = false, answers = {} } = {}) {
  const fake = fakeBridge(answers);
  const frames = manualFrames();
  const card = new TabCardViewModel();
  const drag = new TabDragViewModel({ bridge: fake.bridge, frames, card });
  const vm = new TabStripViewModel({ bridge: fake.bridge, card, drag, reducedMotion: () => reduced });
  fake.emit('onTabsUpdated', [tab(1, { active: true }), tab(2), tab(3)]);
  return { fake, frames, card, drag, vm };
}

/** The ids on the strip, in order, without tabs folding away. */
const ids = (vm: TabStripViewModel) => vm.state.items.filter((i) => !i.closing).map((i) => i.tab.id);

/** A port over a strip that never scrolls. */
const PORT = { scrollLeft: () => 0, edges: () => ({ left: 0, right: 1000 }), scrollBy: () => {} };

/** Three 100px slots. */
const SLOTS = [0, 100, 200].map((left) => ({ left, width: 100 }));

/** Lets the fake bridge's promises settle. */
const settle = () => new Promise<void>((resolve) => setImmediate(resolve));

describe('TabStripViewModel', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it("follows the main process's order, which a drag or a shortcut changed", () => {
    const { fake, vm } = strip();
    fake.emit('onTabsUpdated', [tab(3), tab(1, { active: true }), tab(2)]);
    assert.deepEqual(ids(vm), [3, 1, 2]);
  });

  it('folds a closed tab away: no longer a tab at once, gone after the fold', () => {
    const { fake, vm } = strip();
    fake.emit('onTabsUpdated', [tab(1, { active: true }), tab(3)]);
    assert.equal(vm.state.items.find((i) => i.tab.id === 2)?.closing, true);
    mock.timers.tick(C.TAB_CLOSE_MS);
    assert.equal(
      vm.state.items.find((i) => i.tab.id === 2),
      undefined,
    );
  });

  it('removes a closed tab at once with reduced motion', () => {
    const { fake, vm } = strip({ reduced: true });
    fake.emit('onTabsUpdated', [tab(1, { active: true })]);
    assert.deepEqual(
      vm.state.items.map((i) => i.tab.id),
      [1],
    );
  });

  it('grows in tabs opened after the first draw, until their animation ends', () => {
    const { fake, vm } = strip();
    assert.ok(
      vm.state.items.every((i) => !i.opening),
      'the first tabs are just there',
    );
    fake.emit('onTabsUpdated', [tab(1, { active: true }), tab(2), tab(3), tab(4)]);
    assert.equal(vm.state.items.at(-1)?.opening, true);
    vm.opened(4);
    assert.equal(vm.state.items.at(-1)?.opening, false);
  });

  it('grows nothing in with reduced motion', () => {
    const { fake, vm } = strip({ reduced: true });
    fake.emit('onTabsUpdated', [tab(1, { active: true }), tab(2), tab(3), tab(4)]);
    assert.equal(vm.state.items.at(-1)?.opening, false);
  });

  it('shares the room evenly, and says when the tabs overflow it', () => {
    const { vm } = strip();
    vm.setRoom(600);
    assert.deepEqual(stripLayout(vm.state), { width: 200, size: 'normal', overflowing: false });
    vm.setRoom(100);
    assert.deepEqual(stripLayout(vm.state), { width: 44, size: 'tiny', overflowing: true });
  });

  it('holds tab widths after a close with the mouse until the pointer leaves the strip', () => {
    const { fake, vm } = strip();
    vm.setRoom(600);
    vm.closeByMouse(2);
    assert.deepEqual(fake.called('closeTab'), [[2]]);
    fake.emit('onTabsUpdated', [tab(1, { active: true }), tab(3)]);
    assert.equal(stripLayout(vm.state).width, 200, 'the next close button lands under the pointer');
    vm.release();
    assert.equal(stripLayout(vm.state).width, 240);
  });

  it('hides the hover card on a close with the mouse and on the menu', () => {
    const { card, vm } = strip();
    card.hover(tab(1), 0);
    mock.timers.tick(C.TAB_CARD_DELAY_MS);
    vm.closeByMouse(3);
    assert.equal(card.state.tab, null);
    card.hover(tab(1), 0);
    mock.timers.tick(C.TAB_CARD_DELAY_MS);
    vm.menu(2);
    assert.equal(card.state.tab, null);
  });

  it('closes on Delete, and asks for the native menu on right-click', () => {
    const { fake, vm } = strip();
    vm.close(1);
    vm.menu(2);
    assert.deepEqual(fake.called('closeTab'), [[1]]);
    assert.deepEqual(fake.called('showTabMenu'), [[2]]);
  });

  it('moves between tabs with the arrows, Home and End, showing the one it lands on', () => {
    const { fake, vm } = strip();
    assert.equal(vm.step('ArrowRight', 1), 2);
    assert.equal(vm.step('ArrowLeft', 1), 3, 'wraps round');
    assert.equal(vm.step('End', 1), 3);
    assert.equal(vm.step('Home', 3), 1);
    assert.deepEqual(fake.called('activateTab'), [[2], [3], [3], [1]]);
  });

  it('leaves other keys alone', () => {
    const { fake, vm } = strip();
    assert.equal(vm.step('a', 1), null);
    assert.equal(fake.called('activateTab').length, 0);
  });

  it('keeps the shown order and held widths while a press lasts', () => {
    const { fake, drag, vm } = strip();
    vm.setRoom(600);
    vm.closeByMouse(3);
    drag.start({ id: 1, pointerId: 1, x: 0, slots: SLOTS, index: 0 }, PORT);
    fake.emit('onTabsUpdated', [tab(2), tab(1, { active: true })]);
    vm.release();
    assert.deepEqual(ids(vm), [1, 2]);
    assert.equal(vm.state.frozen, 200);
  });

  it('reorders at once on a drop and tells the main process', () => {
    const { fake, drag, vm } = strip();
    drag.start({ id: 1, pointerId: 1, x: 0, slots: SLOTS, index: 0 }, PORT);
    drag.move(1, 110);
    vm.endPress(1);
    assert.deepEqual(ids(vm), [2, 1, 3]);
    assert.deepEqual(fake.called('moveTab'), [[1, 1]]);
    assert.deepEqual(vm.state.drop, { id: 1, to: 1, offset: 10 });
  });

  it("goes back to the main process's order when it refuses the move", async () => {
    const { drag, vm } = strip({ answers: { moveTab: () => Promise.reject(new Error('agent drives')) } });
    drag.start({ id: 1, pointerId: 1, x: 0, slots: SLOTS, index: 0 }, PORT);
    drag.move(1, 110);
    vm.endPress(1);
    await settle();
    assert.deepEqual(ids(vm), [1, 2, 3]);
  });

  it('only shows a tab on a plain click, never reordering', () => {
    const { fake, drag, vm } = strip();
    drag.start({ id: 2, pointerId: 1, x: 0, slots: SLOTS, index: 1 }, PORT);
    vm.endPress(1);
    assert.deepEqual(fake.called('activateTab'), [[2]]);
    assert.equal(fake.called('moveTab').length, 0);
    assert.deepEqual(ids(vm), [1, 2, 3]);
  });

  it('opens a new tab', () => {
    const { fake, vm } = strip();
    vm.newTab();
    assert.equal(fake.called('newTab').length, 1);
  });

  it('stops following the tab list and its fold timers on dispose', () => {
    const { fake, vm } = strip();
    fake.emit('onTabsUpdated', [tab(1, { active: true })]);
    vm.dispose();
    assert.equal(fake.listeners('onTabsUpdated'), 0);
    mock.timers.tick(C.TAB_CLOSE_MS);
    assert.equal(vm.state.items.length, 3, 'nothing changes after dispose');
  });
});
