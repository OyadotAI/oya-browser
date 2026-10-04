/**
 * Unit tests for the studio's state (studio-view-model.ts, studio-model.ts):
 * labels, the save card, what disables which control, the stage, the
 * selection while recording, the workflow picker, tabs and messages.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { studioApp, STEPS } from '../studio-app.ts';
import {
  finishCopy,
  finishTitle,
  isDisabled,
  plural,
  saveHint,
  saveLabel,
  stepLimit,
  studioMode,
  toggleTitle,
  withCurrent,
  type ControlId,
} from '../../../../../../src/renderer/features/studio/model/studio-model.ts';

/** `n` copies of a navigate step. */
const many = (n: number) => Array.from({ length: n }, (_, i) => ({ ...STEPS[0], id: `s${i}` }));

describe('StudioViewModel', () => {
  it('says "Saved" only while the draft is what was saved, and "Save changes" after an edit', async () => {
    const { vm, ws, push } = await studioApp();
    ws.edit({ type: 'metadata', name: 'lookup' });
    Object.assign(ws.draft, { publishedAt: 1, publishedRevision: ws.draft.revision });
    await push();
    assert.equal(saveLabel(vm.state, ws.draft), 'Saved');
    assert.equal(isDisabled(vm.state, 'record-save'), true);
    ws.edit({ type: 'metadata', name: 'lookup-2' });
    await push();
    assert.equal(saveLabel(vm.state, ws.draft), 'Save changes');
    assert.equal(isDisabled(vm.state, 'record-save'), false);
  });

  it('lists the draft being edited even before it is stored, so the picker is never blank', async () => {
    const { vm } = await studioApp({ steps: [] });
    const s = vm.state.snapshot!;
    assert.equal(withCurrent(s.library, s.draft)[0].name, 'Untitled workflow');
  });

  it('hides empty drafts left behind in the library, but never the one being edited', async () => {
    const { vm, ws, push } = await studioApp();
    ws.store.save({ id: 'stray', name: 'Untitled workflow', updatedAt: 1, steps: [] });
    ws.edit({ type: 'metadata', name: 'mine' });
    await push();
    const ids = withCurrent(vm.state.snapshot!.library, ws.draft).map((item) => item.id);
    assert.equal(ids.includes('stray'), false);
    assert.equal(ids.includes(ws.draft.id), true);
  });

  it('selects and follows the newest step while recording, and keeps the pick otherwise', async () => {
    const { vm, ws, push } = await studioApp();
    vm.select('a');
    ws.capture(
      [...ws.draft.steps, { id: 'c', action: 'click', candidates: [{ kind: 'text', value: 'Next' }] }],
      [],
      true,
    );
    await push();
    assert.equal(vm.state.selected, 'c');
    assert.equal(vm.state.followSteps, true);
    vm.followed();
    ws.capture(ws.draft.steps, [], false);
    await push();
    assert.equal(vm.state.followSteps, false);
    vm.select('a');
    await push();
    assert.equal(vm.state.selected, 'a');
  });

  it('selects the first step when the selected one is gone', async () => {
    const { vm, ws, push } = await studioApp();
    vm.select('b');
    ws.edit({ type: 'delete', id: 'b' });
    await push();
    assert.equal(vm.state.selected, 'a');
  });

  it('tells the shell whether a workflow is being recorded', async () => {
    const { ws, push, shell } = await studioApp();
    ws.capture(ws.draft.steps, [], true);
    await push();
    assert.equal(shell.state.recording, true);
    ws.capture(ws.draft.steps, [], false);
    await push();
    assert.equal(shell.state.recording, false);
  });

  it('warns as a recording nears the step limit, and says why it stopped at it', async () => {
    assert.equal(stepLimit(10, true), '');
    assert.equal(stepLimit(460, false), '', 'not recording, under the limit');
    assert.equal(
      stepLimit(460, true),
      '460 of 500 steps. Recording stops at 500: finish this part, save it, and record the rest as a second workflow.',
    );
    assert.match(stepLimit(500, false), /^Recording stopped at the 500-step limit\./);
    const { vm, ws, push } = await studioApp({ steps: many(10) });
    ws.capture(many(460), [], true);
    await push();
    assert.equal(studioMode(vm.state).recording, true);
  });

  it('shows the record shortcut on the button, since it is no longer the one people guess', () => {
    assert.equal(toggleTitle('MacIntel'), 'Start or stop recording (⌘⌥R)');
    assert.equal(toggleTitle('Win32'), 'Start or stop recording (Ctrl+Alt+R)');
  });

  it('counts steps in words, with one step singular', () => {
    assert.equal(plural(1, 'step'), '1 step');
    assert.equal(plural(3, 'step'), '3 steps');
  });

  it('lists what blocks a test run and turns Test run off', async () => {
    const { vm, ws, fake } = await studioApp();
    fake.emit('onWorkspace', { ...ws.snapshot(), issues: [{ message: 'Start with a page to open' }] });
    assert.deepEqual(vm.state.snapshot!.issues, [{ message: 'Start with a page to open' }]);
    assert.equal(isDisabled(vm.state, 'record-validate'), true);
    assert.equal(saveHint(vm.state), 'Fix the steps marked ! before saving.');
  });

  it('keeps Test run and Save off until the workspace has answered', async () => {
    const { vm } = await studioApp({ commands: { get: () => undefined } });
    assert.equal(vm.state.snapshot, undefined);
    assert.equal(isDisabled(vm.state, 'record-validate'), true);
    assert.equal(isDisabled(vm.state, 'record-save'), true);
    assert.equal(isDisabled(vm.state, 'record-toggle'), false, 'recording can still start');
  });

  it('ignores a workspace push without a draft', async () => {
    const { vm, fake } = await studioApp();
    const before = vm.state.snapshot;
    fake.emit('onWorkspace', { draft: null });
    assert.equal(vm.state.snapshot, before);
  });

  it('turns off New and the workflow list while the draft cannot be stored, and says why', async () => {
    const { vm } = await studioApp({ storage: { failSave: true } });
    assert.equal(isDisabled(vm.state, 'record-clear'), true);
    assert.equal(isDisabled(vm.state, 'draft-library'), true);
    assert.ok(vm.state.snapshot!.storageError);
    assert.match(finishCopy(vm.state, vm.state.snapshot!.draft), /only in memory/);
  });

  it('turns Save off and says why as soon as the name cannot be published', async () => {
    const { vm } = await studioApp();
    vm.typeField('name', 'my workflow');
    assert.equal(isDisabled(vm.state, 'record-save'), true);
    assert.match(saveHint(vm.state), /letters, numbers/);
    vm.typeField('name', 'my-workflow');
    assert.equal(isDisabled(vm.state, 'record-save'), false);
  });

  it('turns Save off offline, and says the draft works offline', async () => {
    const { vm } = await studioApp({ connected: false });
    vm.typeField('name', 'lookup');
    assert.equal(isDisabled(vm.state, 'record-save'), true);
    assert.match(saveHint(vm.state), /^Connect this browser/);
  });

  it('locks the name, description, variables and editing while recording, in the recording stage', async () => {
    const { vm, ws, push } = await studioApp();
    ws.capture(ws.draft.steps, [], true);
    await push();
    const locked: ControlId[] = [
      'record-name',
      'record-desc',
      'variable-add',
      'step-add',
      'record-clear',
      'run-history',
    ];
    for (const id of locked) assert.equal(isDisabled(vm.state, id), true, id);
    assert.equal(studioMode(vm.state).stage, 'recording');
  });

  it('names its stage: empty, captured, running', async () => {
    const empty = await studioApp({ steps: [] });
    assert.equal(studioMode(empty.vm.state).stage, 'empty');
    const { vm, ws, push } = await studioApp();
    assert.equal(studioMode(vm.state).stage, 'captured');
    await ws.start({});
    await push();
    assert.equal(studioMode(vm.state).stage, 'running');
  });

  it('clears the last draft’s messages and its "just recorded" note when another draft is opened', async () => {
    const { vm, ws, push } = await studioApp();
    vm.say('Saved “x” to Oya.', false, 'save-result');
    vm.mark({ justFinished: true });
    ws.edit({ type: 'new' });
    await push();
    assert.equal(vm.state.messages['save-result'].text, '');
    assert.equal(finishTitle(vm.state), 'Save to Oya');
  });

  it('leaves the name alone while it is typed in, and sends a changed name when the field is left', async () => {
    const { vm, ws, push } = await studioApp();
    vm.focusField('name');
    vm.typeField('name', 'typed');
    await push();
    assert.equal(vm.state.name, 'typed');
    vm.blurField();
    await new Promise((resolve) => setImmediate(resolve));
    await push();
    assert.equal(ws.draft.name, 'typed');
  });

  it('moves between the studio tabs with the arrows, wrapping round', async () => {
    const { vm } = await studioApp();
    assert.equal(vm.tabKey('ArrowLeft'), 'code');
    assert.equal(vm.tabKey('ArrowRight'), 'steps');
    assert.equal(vm.tabKey('Enter'), undefined);
    assert.equal(vm.state.tab, 'steps');
  });

  it('shows a command’s error without Electron’s wrapper, in the slot asked for', async () => {
    const commands = {
      undo: () => {
        throw new Error("Error invoking remote method 'workspace': Error: Nothing to undo");
      },
    };
    const { vm } = await studioApp({ commands });
    assert.equal(await vm.command({ type: 'undo' }, 'code-result'), undefined);
    assert.deepEqual(vm.state.messages['code-result'], { text: 'Nothing to undo', error: true });
  });

  it('runs commands one at a time, a later one built on the answer to the one before', async () => {
    const { vm, ws } = await studioApp();
    const first = vm.command({ type: 'metadata', name: 'one', description: '' });
    const second = vm.command(() => ({
      type: 'metadata',
      name: vm.state.snapshot!.draft.name + '-two',
      description: '',
    }));
    await Promise.all([first, second]);
    assert.equal(ws.draft.name, 'one-two');
  });

  it('stops following the workspace when disposed', async () => {
    const { vm, fake } = await studioApp();
    vm.dispose();
    assert.equal(fake.listeners('onWorkspace'), 0);
  });
});
