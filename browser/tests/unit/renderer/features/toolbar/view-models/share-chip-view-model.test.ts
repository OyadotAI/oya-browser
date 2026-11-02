/**
 * Unit tests for the share chip: shown only on a tab that is neither shared nor
 * the call's, worded by host with fallbacks, and its click asks main to go to the call.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ShareChipViewModel } from '../../../../../../src/renderer/features/toolbar/view-models/share-chip-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** A share of tab 1 by a call in tab 2. */
const SHARE = { sharedTabId: 1, callTabId: 2, shared: 'docs.test', call: 'meet.test' };
/** A strip of three tabs with `active` on screen. */
const strip = (active: number) => [1, 2, 3].map((id) => ({ id, active: id === active }));

/** A chip over a fake bridge, with the strip on `active`. */
function chip(active: number, share: unknown = SHARE) {
  const fake = fakeBridge();
  const vm = new ShareChipViewModel(fake.bridge as any);
  fake.emit('onTabsUpdated', strip(active));
  fake.emit('onShareChip', share);
  return { fake, vm };
}

it('is hidden while nothing is shared', () => {
  assert.equal(chip(3, null).vm.state.visible, false);
});

it('is hidden on the shared tab and on the call tab', () => {
  assert.equal(chip(1).vm.state.visible, false);
  assert.equal(chip(2).vm.state.visible, false);
});

it('shows on any other tab, naming the shared host and the call', () => {
  const { state } = chip(3).vm;
  assert.equal(state.visible, true);
  assert.equal(state.status, 'Sharing docs.test');
  assert.equal(state.label, "Go to call on meet.test. You're sharing docs.test.");
  assert.match(state.title, /can't move a live share to another tab/);
});

it('names a non-web shared page "an Oya page" and a non-web call "the call"', () => {
  const { state } = chip(3, { ...SHARE, shared: null, call: null }).vm;
  assert.equal(state.status, 'Sharing an Oya page');
  assert.equal(state.label, "Go to call on the call. You're sharing an Oya page.");
  assert.match(state.title, /^You're sharing an Oya page with the call\. /);
});

it('follows the active tab and the end of the share', () => {
  const { fake, vm } = chip(3);
  fake.emit('onTabsUpdated', strip(2));
  assert.equal(vm.state.visible, false);
  fake.emit('onTabsUpdated', strip(3));
  assert.equal(vm.state.visible, true);
  fake.emit('onShareChip', null);
  assert.equal(vm.state.visible, false);
});

it('its click asks main to go to the call, naming no tab', () => {
  const { fake, vm } = chip(3);
  vm.goToCall();
  assert.deepEqual(fake.called('goToShareCall'), [[]]);
});
