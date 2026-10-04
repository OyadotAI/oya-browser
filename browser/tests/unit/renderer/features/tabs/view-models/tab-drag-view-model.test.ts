/**
 * Unit tests for dragging a tab: a press shows it, a small move lifts it, it
 * follows the pointer within the strip with its neighbours sliding, the strip
 * scrolls under it near an edge, and Escape or a lost pointer put it back.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  TabDragViewModel,
  slideFor,
} from '../../../../../../src/renderer/features/tabs/view-models/tab-drag-view-model.ts';
import { TabCardViewModel } from '../../../../../../src/renderer/features/tabs/view-models/tab-card-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeBridge, manualFrames } from '../../../support/bridge.ts';

/** Four 100px slots. */
const SLOTS = [0, 100, 200, 300].map((left) => ({ left, width: 100 }));

/** A strip showing 300px of its tabs, that can scroll. */
function port() {
  const strip = { scroll: 0 };
  return {
    strip,
    port: {
      scrollLeft: () => strip.scroll,
      edges: () => ({ left: 0, right: 300 }),
      scrollBy: (px: number) => void (strip.scroll += px),
    },
  };
}

/** A drag over a fake bridge and frames, with tab 1 (index 0) pressed at x=50. */
function pressed() {
  const fake = fakeBridge();
  const frames = manualFrames();
  const card = new TabCardViewModel();
  const vm = new TabDragViewModel({ bridge: fake.bridge, frames, card });
  const { strip, port: p } = port();
  vm.start({ id: 1, pointerId: 7, x: 50, slots: SLOTS, index: 0 }, p);
  return { fake, frames, card, vm, strip };
}

describe('TabDragViewModel', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('shows the pressed tab at once', () => {
    const { fake, vm } = pressed();
    assert.deepEqual(fake.called('activateTab'), [[1]]);
    assert.equal(vm.state.id, 1);
    assert.equal(vm.state.lifted, false);
  });

  it('lifts only past the threshold', () => {
    const { vm } = pressed();
    vm.move(7, 50 + C.TAB_DRAG_THRESHOLD - 1);
    assert.equal(vm.state.lifted, false);
    vm.move(7, 50 + C.TAB_DRAG_THRESHOLD);
    assert.equal(vm.state.lifted, true);
  });

  it('hides the hover card when it lifts', () => {
    const { card, vm } = pressed();
    card.hover({ id: 2, title: '', url: '', home: false, favicon: null, active: false }, 0);
    mock.timers.tick(C.TAB_CARD_DELAY_MS);
    vm.move(7, 120);
    assert.equal(card.state.tab, null);
  });

  it('follows the pointer within the strip and slides the tabs it passes', () => {
    const { vm } = pressed();
    vm.move(7, 160);
    assert.equal(vm.state.dx, 110);
    assert.deepEqual(vm.state.shifts, [0, -100, 0, 0]);
    vm.move(7, 5000);
    assert.equal(vm.state.dx, 300, 'never past the last slot');
    assert.equal(slideFor(vm.state, 1, 0), 300);
    assert.equal(slideFor(vm.state, 2, 1), -100);
  });

  it('draws no slides before it lifts', () => {
    const { vm } = pressed();
    assert.equal(slideFor(vm.state, 1, 0), undefined);
  });

  it('ignores another pointer', () => {
    const { vm } = pressed();
    vm.move(8, 300);
    assert.equal(vm.state.lifted, false);
    assert.equal(vm.release(8), null);
    assert.equal(vm.state.id, 1, 'the press still lasts');
  });

  it('scrolls the strip each frame while the pointer is near its edge, and keeps the tab under it', () => {
    const { frames, strip, vm } = pressed();
    vm.move(7, 290);
    const dx = vm.state.dx;
    frames.run();
    assert.ok(strip.scroll > 0);
    assert.ok(vm.state.dx > dx);
    assert.equal(frames.waiting, 1, 'the next frame is asked for');
  });

  it('answers where a dragged tab lands and how far it was let go from there', () => {
    const { frames, vm } = pressed();
    vm.move(7, 160);
    assert.deepEqual(vm.release(7), { id: 1, to: 1, offset: 10 });
    assert.equal(vm.state.id, null);
    assert.equal(frames.waiting, 0, 'scrolling stops');
  });

  it('answers nothing for a click, or a drag back to where it started', () => {
    const click = pressed();
    assert.equal(click.vm.release(7), null);
    const back = pressed();
    back.vm.move(7, 80);
    assert.equal(back.vm.release(7), null);
  });

  it('puts the tab back on Escape during a drag, and leaves Escape alone otherwise', () => {
    const { vm } = pressed();
    assert.equal(vm.cancel(), false, 'a press that has not lifted');
    vm.move(7, 210);
    assert.equal(vm.cancel(), true);
    assert.equal(vm.state.lifted, false);
    assert.equal(vm.release(7), null, 'nothing drops after');
  });

  it('stops scrolling on dispose', () => {
    const { frames, vm } = pressed();
    vm.move(7, 210);
    vm.dispose();
    assert.equal(frames.waiting, 0);
  });
});
