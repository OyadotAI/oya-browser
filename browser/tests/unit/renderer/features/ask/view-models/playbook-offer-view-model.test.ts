/**
 * Unit tests for Save as playbook under an Ask reply: enabled only for a run
 * with an action to replay, a name the server accepts, the saved confirmation,
 * the failure paths, and its tool list kept equal to the server's.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { PlaybookOfferViewModel } from '../../../../../../src/renderer/features/ask/view-models/playbook-offer-view-model.ts';
import { REPLAYABLE_TOOLS } from '../../../../../../src/renderer/features/ask/model/constants.ts';
import { canReplay, suggestName } from '../../../../../../src/renderer/features/ask/model/reply.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** An offer over a fake bridge whose saveChatPlaybook answers `answer`. */
function offer(answer: unknown, canSave = true) {
  const fake = fakeBridge({ saveChatPlaybook: answer });
  return { fake, vm: new PlaybookOfferViewModel(fake.bridge, 'check-my-inbox', canSave) };
}

/** The quoted tool names in the Set literal that follows `name` in `file` (relative to the repository). */
function toolSet(file: string, name: string): string[] {
  const text = fs.readFileSync(path.join(import.meta.dirname, '../../../../../../..', file), 'utf8');
  const body = text.slice(text.indexOf(name)).match(/new Set\(\[([^\]]*)\]/)?.[1] ?? '';
  return [...body.matchAll(/'([a-z_]+)'/g)].map((m) => m[1]).sort();
}

describe('Save as playbook', () => {
  it('cannot be opened for a run that only read pages', () => {
    const { vm } = offer({ ok: true }, false);
    vm.openForm();
    assert.equal(vm.state.stage, 'offer');
  });

  it('is enabled once the run acted on a page, not for visiting and reading alone', () => {
    assert.equal(canReplay([{ name: 'navigate' }, { name: 'analyze_page' }]), false);
    assert.equal(canReplay([{ name: 'navigate' }, { name: 'analyze_page' }, { name: 'click' }]), true);
    assert.equal(canReplay([{ name: 'switch_tab' }, { name: 'keyboard_type' }]), true);
  });

  it('suggests a name from the prompt', () => {
    assert.equal(suggestName('Check my inbox!'), 'check-my-inbox');
    assert.equal(suggestName('!!!'), 'agent-run');
  });

  it('saves the run under the typed name, and shows it saved', async () => {
    const { fake, vm } = offer({ ok: true });
    vm.openForm();
    vm.setName('  inbox-check ');
    await vm.save();
    assert.deepEqual(fake.called('saveChatPlaybook'), [['inbox-check']]);
    assert.deepEqual([vm.state.stage, vm.state.savedAs], ['saved', 'inbox-check']);
  });

  it('refuses a name the server would not accept, without asking it', async () => {
    const { fake, vm } = offer({ ok: true });
    vm.openForm();
    vm.setName('my playbook!');
    await vm.save();
    assert.match(vm.state.error, /letters, numbers, hyphens or underscores/);
    assert.equal(fake.called('saveChatPlaybook').length, 0);
  });

  it('shows why the server refused, and lets the person try again', async () => {
    const { vm } = offer({ error: 'Not connected to server' });
    vm.openForm();
    await vm.save();
    assert.deepEqual([vm.state.stage, vm.state.error], ['form', 'Not connected to server']);
  });

  it('shows why a failed call failed', async () => {
    const { vm } = offer(() => Promise.reject(new Error('boom')));
    vm.openForm();
    await vm.save();
    assert.equal(vm.state.error, 'boom');
  });

  it('goes back to the button on Cancel', () => {
    const { vm } = offer({ ok: true });
    vm.openForm();
    vm.cancel();
    assert.equal(vm.state.stage, 'offer');
  });

  it('knows exactly the tools the server records', () => {
    const server = toolSet('server/src/modules/agent/recorder.ts', 'const RECORDED');
    assert.deepEqual([...REPLAYABLE_TOOLS].sort(), server);
    assert.ok(server.length > 0, 'the server list was found');
  });
});
