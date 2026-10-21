/**
 * Unit tests for the address bar's zoom chip: hidden at 100%, showing main's
 * percentage, announcing only a zoom a person made, and a refused reset keeping
 * the number it had.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ZoomChipViewModel } from '../../../../../../src/renderer/features/toolbar/view-models/zoom-chip-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';
import { flush } from '../../../../support/fakes.cjs';

/** A chip over a fake bridge whose reset answers with `reset`. */
function chip(reset: unknown = undefined) {
  const fake = fakeBridge({ resetZoom: reset });
  return { fake, vm: new ZoomChipViewModel(fake.bridge as any) };
}

it('is hidden at 100%', () => {
  const { fake, vm } = chip();
  assert.equal(vm.shown, false);
  fake.emit('onPageZoom', { percent: 100, announce: false });
  assert.equal(vm.shown, false);
});

it("shows main's percentage", () => {
  const { fake, vm } = chip();
  fake.emit('onPageZoom', { percent: 110, announce: false });
  assert.deepEqual([vm.shown, vm.state.percent], [true, 110]);
});

it('announces only when main says a person zoomed', () => {
  const { fake, vm } = chip();
  fake.emit('onPageZoom', { percent: 131, announce: false });
  assert.equal(vm.state.announcement, '', 'a tab switch or a new page is silent');
  fake.emit('onPageZoom', { percent: 110, announce: true });
  assert.equal(vm.state.announcement, 'Zoom 110%');
  fake.emit('onPageZoom', { percent: 100, announce: true });
  assert.equal(vm.state.announcement, 'Zoom reset to 100%');
});

it('a reset asks main, and a refused one keeps the number', async () => {
  const { fake, vm } = chip(() => Promise.reject(new Error('Switch to Human Mode')));
  fake.emit('onPageZoom', { percent: 120, announce: false });
  vm.reset();
  await flush();
  assert.equal(fake.called('resetZoom').length, 1);
  assert.equal(vm.state.percent, 120);
});
