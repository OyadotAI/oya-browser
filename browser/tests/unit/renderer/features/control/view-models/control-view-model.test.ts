/**
 * Unit tests for the control bar: what it says in each mode, its buttons,
 * taking and handing back control, errors that last until the state moves,
 * and the watch-only guard: page actions are refused while the agent drives,
 * except Start recording, which takes control first.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  ControlViewModel,
  blockedActions,
  controlLook,
} from '../../../../../../src/renderer/features/control/view-models/control-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** The agent holds the browser and a person may take it. */
const AGENT = { mode: 'agent', interactive: false, supported: true, connected: true };
/** The person holds it. */
const MINE = { mode: 'human', mine: true, interactive: true, supported: true, connected: true };

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** A control bar over a fake bridge answering from `answers`. */
async function bar(answers: Record<string, unknown> = {}) {
  const fake = fakeBridge({ getControlState: AGENT, ...answers });
  const vm = new ControlViewModel(fake.bridge);
  await settle();
  return { fake, vm, look: () => controlLook(vm.state) };
}

describe('the control status', () => {
  it('says offline, with no buttons, before any state has come', () => {
    const look = controlLook({ control: null, error: '', pending: '' });
    assert.deepEqual(
      [look.mode, look.label, look.action.hidden, look.resume.hidden],
      ['offline', 'Offline · your control', true, true],
    );
  });

  it('names who drives: the agent, you, or another operator', async () => {
    const { fake, look } = await bar();
    assert.deepEqual([look().mode, look().label, look().action.text], ['agent', 'Agent control', 'Take control']);
    assert.equal(look().title, 'Page is watch-only. Take control to interact.');
    fake.emit('onControlState', MINE);
    assert.deepEqual([look().label, look().action.text], ['You’re in control', 'Release to agent']);
    fake.emit('onControlState', { ...MINE, mine: false, interactive: false });
    assert.deepEqual([look().label, look().action.hidden], ['Another operator', true]);
  });

  it('shows a handoff under way as taking, and handing back as returning', async () => {
    const { fake, look } = await bar();
    fake.emit('onControlState', { ...AGENT, taking: true });
    assert.deepEqual([look().mode, look().label], ['taking', 'Taking control…']);
    fake.emit('onControlState', { ...MINE, busy: true, busyAction: 'return' });
    assert.deepEqual([look().label, look().action.disabled], ['Returning control…', true]);
  });

  it('says it is checking for a mode it does not know', async () => {
    const { fake, look } = await bar();
    fake.emit('onControlState', { mode: 'toString' });
    assert.equal(look().label, 'Checking control…');
  });

  it('offers Take control only when handoffs are possible', async () => {
    const { fake, look } = await bar();
    fake.emit('onControlState', { mode: 'agent', supported: false });
    assert.equal(look().action.hidden, true);
    fake.emit('onControlState', { mode: 'agent', localClients: 1 });
    assert.equal(look().action.hidden, false);
  });

  it('offers Return to agent after a pause, and hands control back with it', async () => {
    const { fake, vm, look } = await bar({ changeControl: { state: AGENT } });
    fake.emit('onControlState', { mode: 'paused', supported: true });
    assert.deepEqual([look().label, look().resume.hidden], ['Automation paused', false]);
    await vm.resume();
    assert.deepEqual(fake.called('changeControl'), [['return']]);
  });
});

describe('taking and handing back control', () => {
  it('header takeover asks for confirmation without acquiring implicitly, then releases human control', async () => {
    const { fake, vm } = await bar({ changeControl: { state: AGENT } });
    await vm.toggle();
    assert.equal(fake.called('requestTakeover').length, 1);
    assert.deepEqual(fake.called('changeControl'), []);
    fake.emit('onControlState', MINE);
    await vm.toggle();
    assert.deepEqual(fake.called('changeControl'), [['return']]);
  });

  it('shows why a takeover failed until the state moves on', async () => {
    const { fake, vm, look } = await bar({ changeControl: { error: 'Another operator has control', state: AGENT } });
    await vm.acquire();
    assert.deepEqual([look().label, look().title], ['Another operator has control', 'Another operator has control']);
    fake.emit('onControlState', AGENT);
    assert.equal(look().label, 'Another operator has control', 'the same state keeps the error');
    fake.emit('onControlState', { ...AGENT, revision: 2 });
    assert.equal(look().label, 'Agent control');
  });

  it('says so when the change could not be asked for', async () => {
    const { vm } = await bar({ requestTakeover: () => Promise.reject(new Error('gone')) });
    await vm.toggle();
    assert.equal(vm.state.error, 'Could not change control.');
    assert.equal(vm.state.pending, '');
  });

  it('disables the button while its change is under way', async () => {
    let answer: (v: unknown) => void = () => {};
    const { vm, look } = await bar({ requestTakeover: () => new Promise((resolve) => (answer = resolve)) });
    const change = vm.toggle();
    assert.equal(look().action.disabled, true);
    answer({ state: MINE });
    await change;
    assert.equal(look().action.disabled, false);
  });

  it('stops following the main process once disposed', async () => {
    const { fake, vm } = await bar();
    vm.dispose();
    assert.equal(fake.listeners('onControlState'), 0);
  });
});

describe('the control guard', () => {
  it('lets page actions through while you hold control', async () => {
    const { vm } = await bar({ getControlState: MINE });
    assert.deepEqual([vm.guard(false), vm.guard(true)], ['allow', 'allow']);
    assert.deepEqual(blockedActions(vm.state), { pageBlocked: false, recordBlocked: false });
  });

  it('still refuses other page actions while watch-only, and marks them blocked', async () => {
    const { fake, vm } = await bar();
    assert.equal(vm.guard(false), 'refuse');
    assert.equal(blockedActions(vm.state).pageBlocked, true);
    assert.equal(fake.called('changeControl').length, 0);
  });

  it('takes control, then lets Start recording through, without looking disabled', async () => {
    const { fake, vm } = await bar({ changeControl: { state: MINE } });
    assert.equal(blockedActions(vm.state).recordBlocked, false, 'it does not look disabled');
    assert.equal(vm.guard(true), 'take');
    assert.equal(await vm.acquire(), true);
    assert.deepEqual(fake.called('changeControl'), [['acquire']]);
    assert.equal(vm.guard(true), 'allow');
  });

  it('does not record when taking control fails', async () => {
    const { vm, look } = await bar({ changeControl: { error: 'Another operator has control', state: AGENT } });
    assert.equal(await vm.acquire(), false);
    assert.equal(look().label, 'Another operator has control');
  });

  it('refuses Start recording too when control cannot be taken', async () => {
    const { vm } = await bar({ getControlState: { mode: 'human', mine: false, interactive: false, supported: true } });
    assert.equal(vm.guard(true), 'refuse');
    assert.equal(blockedActions(vm.state).recordBlocked, true);
  });

  it('refuses page actions before any control state has come', () => {
    const vm = new ControlViewModel(fakeBridge({ getControlState: new Promise(() => {}) }).bridge);
    assert.equal(vm.guard(false), 'refuse');
  });
});
