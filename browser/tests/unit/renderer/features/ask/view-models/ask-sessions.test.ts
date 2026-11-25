/**
 * Unit tests for one agent panel per tab: each tab keeps its own conversation and
 * run, a run in one tab never blocks or fills another tab's panel, Clear clears the
 * shown tab only, and a closed tab's run is stopped.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { AskSessions } from '../../../../../../src/renderer/features/ask/view-models/ask-sessions.ts';
import { PanelViewModel } from '../../../../../../src/renderer/app/panel/panel-view-model.ts';
import { fakeBridge, manualFrames } from '../../../support/bridge.ts';
import { fakeRead, settle } from '../support.ts';

/** A strip of tabs `ids`, `active` shown. */
const strip = (ids: number[], active: number) => ids.map((id) => ({ id, active: id === active }));

/** Sessions over a fake bridge whose chats wait until answered by hand. */
async function sessions() {
  const pending: Array<(data: unknown) => void> = [];
  const fake = fakeBridge({ sendChat: () => new Promise((resolve) => pending.push(resolve)) });
  const panel = new PanelViewModel(fake.bridge, manualFrames());
  const vm = new AskSessions({ bridge: fake.bridge, panel }, fakeRead);
  const show = async (ids: number[], active: number) => (
    fake.emit('onTabsUpdated', strip(ids, active)),
    await settle()
  );
  const say = async (text: string) => (vm.state.current.setInput(text), void vm.state.current.send(), await settle());
  return { fake, panel, vm, show, say, pending };
}

describe('AskSessions', () => {
  let made: AskSessions | null = null;
  afterEach(() => made?.dispose());

  it('gives each tab its own agent: a run in one tab never blocks starting another in a new tab', async () => {
    const { fake, vm, show, say } = await sessions();
    made = vm;
    await show([1], 1);
    await say('Book the meeting');
    assert.equal(vm.state.current.state.sending, true);
    await show([1, 2], 2);
    assert.equal(vm.state.current.tabId, 2);
    assert.deepEqual(vm.state.current.state.items, [], 'the new tab starts with an empty panel');
    await say('Check prices');
    assert.deepEqual(
      fake.called('sendChat').map(([, , tab]) => tab),
      [1, 2],
    );
    await show([1, 2], 1);
    assert.equal(vm.state.current.state.items[0].content, 'Book the meeting');
  });

  it("clears only the shown tab's conversation", async () => {
    const { panel, vm, show, say, pending } = await sessions();
    made = vm;
    await show([1, 2], 1);
    await say('one');
    pending.shift()?.({ text: 'done one' });
    await show([1, 2], 2);
    await say('two');
    pending.shift()?.({ text: 'done two' });
    await settle();
    panel.clear();
    assert.deepEqual(vm.state.current.state.items, []);
    await show([1, 2], 1);
    assert.equal(vm.state.current.state.items.length, 2);
  });

  it("shows only its own tab's run in the panel, never another tab's steps", async () => {
    const { fake, vm, show, say } = await sessions();
    made = vm;
    await show([1, 2], 2);
    await say('mine');
    fake.emit('onAgentEvent', { runId: 'other', tabId: 1, event: { kind: 'start', task: 'theirs' } });
    fake.emit('onAgentEvent', { runId: 'mine', tabId: 2, event: { kind: 'start', task: 'mine' } });
    fake.emit('onAgentEvent', { runId: 'other', tabId: 1, event: { kind: 'step', line: 'Opening theirs' } });
    fake.emit('onAgentEvent', { runId: 'mine', tabId: 2, event: { kind: 'step', line: 'Opening mine' } });
    assert.deepEqual(
      vm.state.current.run.state.card?.steps.map((s) => s.line),
      ['Opening mine'],
    );
  });

  it("stops a closed tab's run", async () => {
    const { fake, vm, show, say } = await sessions();
    made = vm;
    await show([1, 2], 1);
    await say('long task');
    await show([2], 2);
    assert.deepEqual(fake.called('stopChat'), [[1]]);
  });

  it('stops and disposes the last tab when all tabs close on sign-out', async () => {
    const { fake, vm, show, say } = await sessions();
    made = vm;
    await show([1], 1);
    await say('sign-out task');
    assert.equal(vm.anyBusy(), true);
    await show([], -1);
    assert.deepEqual(fake.called('stopChat'), [[1]]);
    assert.equal(vm.state.current.tabId, null);
    assert.equal(vm.anyBusy(), false);
  });

  it('disposes a pre-tab sending session once its run finishes after the first tab appears', async () => {
    const { vm, show, say, pending } = await sessions();
    made = vm;
    await say('before tabs');
    const preTab = vm.state.current;
    assert.equal(preTab.tabId, null);
    await show([1], 1);
    assert.equal(vm.state.current.tabId, 1);
    assert.equal(vm.anyBusy(), true);
    pending.shift()?.({ text: 'finished' });
    await settle();
    assert.equal(vm.anyBusy(), false);
  });
});
