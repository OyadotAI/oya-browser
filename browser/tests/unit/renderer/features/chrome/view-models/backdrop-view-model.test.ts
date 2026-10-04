/**
 * Unit tests for the page backdrop: a still shows once decoded and the main
 * process hears once a frame has painted it; an older still never shows over
 * a newer one, and none hides it.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  BackdropViewModel,
  stillOf,
} from '../../../../../../src/renderer/features/chrome/view-models/backdrop-view-model.ts';
import { fakeBridge, manualFrames } from '../../../support/bridge.ts';

/** A still for `token`. */
const still = (token: string) => ({
  image: `data:image/png;base64,${token}`,
  bounds: { x: 0, y: 80, width: 800, height: 600 },
  token,
});

/** A backdrop over a fake bridge and frames. */
function backdrop() {
  const fake = fakeBridge();
  const frames = manualFrames();
  return { fake, frames, vm: new BackdropViewModel({ bridge: fake.bridge, frames }) };
}

describe('BackdropViewModel', () => {
  it('shows a decoded still and says so once a frame has painted it', () => {
    const { fake, frames, vm } = backdrop();
    fake.emit('onPageBackdrop', still('t1'));
    assert.equal(vm.state.shown, false, 'not before it is decoded');
    vm.decoded('t1');
    assert.equal(vm.state.shown, true);
    assert.equal(fake.called('backdropReady').length, 0);
    frames.run();
    assert.deepEqual(fake.called('backdropReady'), [['t1']]);
  });

  it('never shows an older still that decoded late', () => {
    const { fake, frames, vm } = backdrop();
    fake.emit('onPageBackdrop', still('t1'));
    fake.emit('onPageBackdrop', still('t2'));
    vm.decoded('t1');
    frames.run();
    assert.equal(vm.state.shown, false);
    assert.equal(fake.called('backdropReady').length, 0);
  });

  it('hides on none', () => {
    const { fake, vm } = backdrop();
    fake.emit('onPageBackdrop', still('t1'));
    vm.decoded('t1');
    fake.emit('onPageBackdrop', null);
    assert.deepEqual(vm.state, { still: null, shown: false });
  });

  it('treats a malformed still as none', () => {
    assert.equal(stillOf({ image: 42 }), null);
    assert.equal(stillOf({ image: 'data:', bounds: null }), null);
    assert.deepEqual(stillOf(still('t3')), still('t3'));
  });

  it('stops listening on dispose', () => {
    const { fake, vm } = backdrop();
    vm.dispose();
    assert.equal(fake.listeners('onPageBackdrop'), 0);
  });
});
