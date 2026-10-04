/**
 * Unit tests for the launch: it plays from the first paint, dissolves on its
 * own, gives way at once to a click or a key, and does not play with reduced
 * motion.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { LaunchViewModel } from '../../../../../../src/renderer/features/chrome/view-models/launch-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';

describe('LaunchViewModel', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('dissolves on its own, then is gone', () => {
    const vm = new LaunchViewModel(false);
    assert.equal(vm.state.phase, 'playing', 'it is up from the first paint');
    mock.timers.tick(C.LAUNCH_MS);
    assert.equal(vm.state.phase, 'leaving');
    mock.timers.tick(C.LAUNCH_LEAVE_MS);
    assert.equal(vm.state.phase, 'gone');
  });

  it('gives way at once to a click or a key', () => {
    const vm = new LaunchViewModel(false);
    vm.end();
    assert.equal(vm.state.phase, 'leaving');
    mock.timers.tick(C.LAUNCH_LEAVE_MS);
    assert.equal(vm.state.phase, 'gone');
  });

  it('dissolves only once', () => {
    const vm = new LaunchViewModel(false);
    vm.end();
    mock.timers.tick(C.LAUNCH_LEAVE_MS);
    vm.end();
    mock.timers.tick(C.LAUNCH_MS);
    assert.equal(vm.state.phase, 'gone');
  });

  it('does not play with reduced motion', () => {
    const vm = new LaunchViewModel(true);
    assert.equal(vm.state.phase, 'gone');
    vm.end();
    assert.equal(vm.state.phase, 'gone');
  });

  it('stops its timers on dispose', () => {
    const vm = new LaunchViewModel(false);
    vm.dispose();
    mock.timers.tick(C.LAUNCH_MS);
    assert.equal(vm.state.phase, 'playing');
  });
});
