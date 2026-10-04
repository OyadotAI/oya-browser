/**
 * Unit tests for the tab hover card: it shows after a rest, at once while one
 * is up, and hiding forgets a card waiting to show.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { TabCardViewModel } from '../../../../../../src/renderer/features/tabs/view-models/tab-card-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';

/** A tab. */
const tab = (id: number) => ({ id, title: `Tab ${id}`, url: '', home: false, favicon: null, active: false });

describe('TabCardViewModel', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('shows after the pointer rests on a tab', () => {
    const vm = new TabCardViewModel();
    vm.hover(tab(1), 40);
    mock.timers.tick(C.TAB_CARD_DELAY_MS - 1);
    assert.equal(vm.state.tab, null);
    mock.timers.tick(1);
    assert.deepEqual(vm.state, { tab: tab(1), left: 40 });
  });

  it('shows the next tab at once while one is up', () => {
    const vm = new TabCardViewModel();
    vm.hover(tab(1), 0);
    mock.timers.tick(C.TAB_CARD_DELAY_MS);
    vm.hover(tab(2), 100);
    assert.equal(vm.state.tab?.id, 2);
  });

  it('forgets a card waiting to show when hidden', () => {
    const vm = new TabCardViewModel();
    vm.hover(tab(1), 0);
    vm.hide();
    mock.timers.tick(C.TAB_CARD_DELAY_MS);
    assert.equal(vm.state.tab, null);
  });

  it('forgets a waiting card on dispose', () => {
    const vm = new TabCardViewModel();
    vm.hover(tab(1), 0);
    vm.dispose();
    mock.timers.tick(C.TAB_CARD_DELAY_MS);
    assert.equal(vm.state.tab, null);
  });
});
