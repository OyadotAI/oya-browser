/**
 * Unit tests for the chrome's sign of an agent at work: it follows the
 * commands actually arriving, not the control mode, and settles after the
 * last one.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { AgentActivityViewModel } from '../../../../../../src/renderer/features/chrome/view-models/activity-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** Activity over a fake bridge, and a way to log one entry. */
function activity() {
  const fake = fakeBridge();
  const vm = new AgentActivityViewModel(fake.bridge);
  const log = (dir: string, type: string) => fake.emit('onDevLog', { ts: 0, dir, type, data: '{}' });
  return { fake, vm, log };
}

describe('AgentActivityViewModel', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('shows the agent at work while commands arrive, then settles', () => {
    const { vm, log } = activity();
    log('in', 'cmd: click');
    assert.equal(vm.state.active, true);
    mock.timers.tick(C.AGENT_ACTIVE_MS);
    assert.equal(vm.state.active, false);
  });

  it('keeps it at work while commands keep coming', () => {
    const { vm, log } = activity();
    log('in', 'cmd: click');
    mock.timers.tick(C.AGENT_ACTIVE_MS - 1);
    log('in', 'cmd: type');
    mock.timers.tick(C.AGENT_ACTIVE_MS - 1);
    assert.equal(vm.state.active, true);
  });

  it('ignores replies and anything that is not a command', () => {
    const { vm, log, fake } = activity();
    log('out', 'result');
    log('in', 'auth_ok');
    fake.emit('onDevLog', null);
    assert.equal(vm.state.active, false);
  });

  it('stops listening and settling on dispose', () => {
    const { fake, vm, log } = activity();
    log('in', 'cmd: click');
    vm.dispose();
    mock.timers.tick(C.AGENT_ACTIVE_MS);
    assert.equal(fake.listeners('onDevLog'), 0);
    assert.equal(vm.state.active, true, 'nothing changes after dispose');
  });
});
