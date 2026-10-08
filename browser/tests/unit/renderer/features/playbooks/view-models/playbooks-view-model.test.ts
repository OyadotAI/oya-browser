/** Saved playbook workflows through the public renderer intents and fake IPC. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { PlaybooksViewModel } from '../../../../../../src/renderer/features/playbooks/view-models/playbooks-view-model.ts';
import { PanelViewModel } from '../../../../../../src/renderer/app/panel/panel-view-model.ts';
import { fakeBridge, manualFrames } from '../../../support/bridge.ts';
const item = { name: 'lookup', variables: ['member'], defaults: { member: '123' }, steps: 2, code: 'navigate();' };
/** A library with a fake main process and owned subscriptions. */
function fixture(t: any, playbooks: (command: any) => unknown) {
  const fake = fakeBridge({ playbooks });
  const panel = new PanelViewModel(fake.bridge, manualFrames());
  const vm = new PlaybooksViewModel(fake.bridge, panel);
  t.after(() => {
    vm.dispose();
    panel.dispose();
  });
  return { vm, fake };
}
it('loads, searches, and opens saved inputs without the console', async (t) => {
  const { vm } = fixture(t, () => ({ playbooks: [item] }));
  await vm.refresh();
  vm.search('LOOK');
  assert.equal(vm.matches.length, 1);
  vm.select('lookup');
  assert.equal(vm.state.values.member, '123');
});
it('passes edited inputs to the current-browser run and prevents duplicate submission', async (t) => {
  const { vm, fake } = fixture(t, (c) =>
    c.action === 'list' ? { playbooks: [item] } : { id: 'r1', status: 'running' },
  );
  await vm.refresh();
  vm.select('lookup');
  vm.value('member', '456');
  await vm.run();
  await vm.run();
  const calls = fake
    .called('playbooks')
    .map(([c]) => c as any)
    .filter((c) => c.action === 'run');
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].values, { member: '456' });
});
it('keeps the library and surfaces an API failure', async (t) => {
  let failed = false;
  const { vm } = fixture(t, () => (failed ? { error: 'Reconnect to Oya' } : { playbooks: [item] }));
  await vm.refresh();
  failed = true;
  await vm.refresh();
  assert.equal(vm.state.items.length, 1);
  assert.match(vm.state.error, /Reconnect/);
});
it('does not report canceled file transfers as saved', async (t) => {
  const { vm } = fixture(t, (c) => (c.action === 'list' ? { playbooks: [] } : { canceled: true }));
  await vm.manage('import');
  assert.equal(vm.state.note, '');
  assert.equal(vm.state.busy, false);
});

it('View playbook waits for an already-loading library before filling defaults', async (t) => {
  let resolve!: (value: unknown) => void;
  const { vm } = fixture(
    t,
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const opening = vm.open('lookup');
  await Promise.resolve();
  resolve({ playbooks: [item] });
  await opening;
  assert.equal(vm.state.values.member, '123');
});

it('does not restore a signed-out project from an old in-flight response', async (t) => {
  let resolve!: (value: unknown) => void;
  const { vm, fake } = fixture(
    t,
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const reading = vm.refresh();
  fake.emit('onModeChanged', 'setup');
  resolve({ playbooks: [item] });
  await reading;
  assert.deepEqual(vm.state.items, []);
});
