/**
 * Unit tests for the studio's actions (studio-actions.ts): recording through
 * the control gate, the record shortcut, the save card after a recording, the
 * test run, adding steps and variables, saving to Oya, export, diagnostics,
 * and Expand.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { studioApp, settle } from '../studio-app.ts';
import {
  expandLabel,
  finishCopy,
  finishTitle,
  isDisabled,
  saveLabel,
} from '../../../../../../src/renderer/features/studio/model/studio-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';

/** The workspace commands sent, by type. */
const sent = (fake: { called(method: string): unknown[][] }) =>
  fake.called('workspace').map(([cmd]) => (cmd as any).type);

describe('StudioActions', () => {
  it('starts a new recording on an empty draft, and resumes one that has steps', async () => {
    const empty = await studioApp({ steps: [] });
    await empty.vm.actions.toggleRecording();
    assert.equal(empty.fake.called('startRecording').length, 1);
    const { vm, fake } = await studioApp({ commands: { 'resume-recording': (_c) => ({}) } });
    await vm.actions.toggleRecording();
    assert.ok(sent(fake).includes('resume-recording'));
  });

  it('records nothing when the control gate refuses', async () => {
    const { vm, fake } = await studioApp({ steps: [], gate: { admit: false } });
    await vm.actions.toggleRecording();
    assert.equal(fake.called('startRecording').length, 0);
  });

  it('says a recording was just captured only right after one, never for a draft opened later', async () => {
    let stop = () => {};
    const { vm, ws, push } = await studioApp({ answers: { stopRecording: () => stop() } });
    assert.equal(finishTitle(vm.state), 'Save to Oya');
    ws.capture(ws.draft.steps, [], true);
    await push();
    stop = () => ws.capture(ws.draft.steps, [], false);
    await vm.actions.toggleRecording();
    assert.equal(finishTitle(vm.state), 'Recording captured');
    assert.equal(vm.state.revealFinish, true, 'the save card comes into view');
    assert.equal(vm.state.tab, 'steps');
    await vm.actions.newDraft();
    ws.capture(structuredClone(ws.draft.steps), [], false);
    await push();
    assert.equal(finishTitle(vm.state), 'Save to Oya');
  });

  it('asks for a name only for a playbook not yet in Oya, and for saving changes after', async () => {
    let stop = () => {};
    const { vm, ws, push } = await studioApp({ answers: { stopRecording: () => stop() } });
    Object.assign(ws.draft, { publishedAt: 1, publishedRevision: ws.draft.revision });
    ws.capture(ws.draft.steps, [], true);
    await push();
    stop = () => ws.capture(ws.draft.steps, [], false);
    await vm.actions.toggleRecording();
    assert.match(finishCopy(vm.state, ws.draft), /Save the changes to update it in Oya/);
  });

  it('never says "Saving…" while a recording is being stopped, and keeps the button off meanwhile', async () => {
    let release = () => {};
    const answers = { stopRecording: () => new Promise<void>((resolve) => (release = resolve)) };
    const { vm, ws, push } = await studioApp({ answers });
    ws.capture(ws.draft.steps, [], true);
    await push();
    const stopping = vm.actions.toggleRecording();
    await settle();
    assert.notEqual(saveLabel(vm.state, ws.draft), 'Saving…');
    assert.equal(isDisabled(vm.state, 'record-toggle'), true);
    ws.capture(ws.draft.steps, [], false);
    release();
    await stopping;
    assert.equal(vm.state.busy, false);
  });

  it('keeps the error when resuming a recording fails', async () => {
    const commands = {
      'resume-recording': () => {
        throw new Error("Error invoking remote method 'workspace': Error: Take control first");
      },
    };
    const { vm } = await studioApp({ commands });
    await vm.actions.toggleRecording();
    assert.deepEqual(vm.state.messages['record-result'], { text: 'Take control first', error: true });
  });

  it('opens the studio on Steps and presses record from the shortcut', async () => {
    const { vm, fake } = await studioApp({ steps: [] });
    vm.selectTab('code');
    await vm.actions.recordShortcut();
    assert.equal(fake.called('toggleDevPanel').length, 1);
    assert.equal(vm.state.tab, 'steps');
    assert.equal(fake.called('startRecording').length, 1);
  });

  it('does nothing on the record shortcut when the record button could not be pressed', async () => {
    const { vm, fake } = await studioApp({ gate: { blocked: true } });
    await vm.actions.recordShortcut();
    assert.equal(sent(fake).includes('resume-recording'), false);
    assert.equal(fake.called('toggleDevPanel').length, 0);
  });

  it('stays on Steps when the test-run prompt is cancelled, and shows Run once a run starts', async () => {
    let started = false;
    const { vm, ws } = await studioApp({
      commands: { validate: async () => (started && (await ws.start({})), ws.snapshot()) },
    });
    await vm.actions.validate();
    assert.equal(vm.state.tab, 'steps');
    started = true;
    await vm.actions.validate();
    assert.equal(vm.state.tab, 'run');
  });

  it('sends the run inputs and speed with a test run, then forgets the secret ones', async () => {
    const steps = [
      { id: 'a', action: 'navigate', url: 'https://x.test/{{where}}' },
      { id: 'b', action: 'type', text: '{{pin}}', candidates: [{ kind: 'css', value: '#pin' }] },
    ];
    const { vm, ws, push, fake } = await studioApp({ steps });
    ws.edit({ type: 'variables', variables: { where: { default: 'home' }, pin: { secret: true } } });
    await push();
    vm.run.setInput('pin', '1234');
    vm.run.setSpeed(1000);
    await vm.actions.validate();
    const cmd = fake.called('workspace').at(-1)?.[0] as any;
    assert.deepEqual([cmd.vars, cmd.slowMo], [{ where: 'home', pin: '1234' }, 1000]);
    assert.deepEqual(vm.state.runInputs, { where: 'home', pin: '' });
  });

  it('keeps the selected step when adding a step is refused, and says why', async () => {
    const commands = {
      add: () => {
        throw new Error('Too many steps');
      },
    };
    const { vm } = await studioApp({ commands });
    vm.select('b');
    await vm.actions.addStep('wait');
    assert.equal(vm.state.selected, 'b');
    assert.equal(vm.state.messages['record-result'].text, 'Too many steps');
  });

  it('selects a step just added after the selected one', async () => {
    const { vm, ws } = await studioApp();
    vm.select('a');
    await vm.actions.addStep('wait');
    assert.equal(ws.draft.steps[1].action, 'wait');
    assert.equal(vm.state.selected, ws.draft.steps[1].id);
  });

  it('shows why a variable could not be added, not how to use it', async () => {
    const commands = {
      variables: () => {
        throw new Error('Variable names must be letters and numbers');
      },
    };
    const { vm } = await studioApp({ commands });
    await vm.actions.addVariable();
    assert.equal(vm.state.messages['record-result'].text, 'Variable names must be letters and numbers');
  });

  it('adds the next free input variable and says how to use it', async () => {
    const { vm, ws } = await studioApp();
    await vm.actions.addVariable();
    await vm.actions.addVariable();
    assert.deepEqual(Object.keys(ws.draft.variables), ['input_1', 'input_2']);
    assert.equal(vm.state.messages['record-result'].text, 'Use {{input_2}} in a step.');
  });

  it('keeps Copy and Save module off until there is code to copy', async () => {
    const { vm } = await studioApp({ steps: [{ id: 'x', action: 'click', candidates: [] }] });
    assert.equal(isDisabled(vm.state, 'record-copy'), true);
    assert.equal(isDisabled(vm.state, 'record-download'), true);
    const ready = await studioApp();
    assert.equal(isDisabled(ready.vm.state, 'record-copy'), false);
  });

  it('copies the code and says so, and says why when the clipboard refuses', async () => {
    const { vm, copied } = await studioApp();
    await vm.actions.copy();
    assert.equal(copied[0], vm.state.snapshot!.code);
    assert.equal(vm.state.messages['code-result'].text, 'Code copied.');
    const refused = await studioApp({ clipboardFails: true });
    await refused.vm.actions.copy();
    assert.deepEqual(refused.vm.state.messages['code-result'], { text: 'denied', error: true });
  });

  it('says the module was exported only when the file was written', async () => {
    let saved = true;
    const { vm, fake } = await studioApp({ answers: { exportPlaywright: () => ({ saved }) } });
    await vm.actions.download();
    assert.equal(vm.state.messages['code-result'].text, 'Playwright module exported.');
    assert.equal((fake.called('exportPlaywright')[0][0] as any).code, vm.state.snapshot!.code);
    saved = false;
    await vm.actions.download();
    assert.equal(vm.state.messages['code-result'].text, '');
  });

  it('says which JSON was saved, and nothing when the dialog was cancelled', async () => {
    const { vm, ws } = await studioApp();
    await vm.actions.exportJson('chrome');
    assert.equal(vm.state.messages['code-result'].text, 'Saved for Chrome Recorder.');
    const cancelled = await studioApp({ commands: { 'export-json': () => ({ ...ws.snapshot(), exported: false }) } });
    await cancelled.vm.actions.exportJson('oya');
    assert.equal(cancelled.vm.state.messages['code-result'].text, '');
  });

  it('opens an imported workflow on the Steps tab', async () => {
    const { vm, ws } = await studioApp({
      commands: {
        'import-json': () => (ws.edit({ type: 'import', draft: { name: 'file', steps: [] } }), ws.snapshot()),
      },
    });
    vm.selectTab('code');
    await vm.actions.importJson();
    assert.equal(vm.state.tab, 'steps');
    assert.equal(vm.state.snapshot!.draft.name, 'file');
  });

  it('says so when diagnostics are saved', async () => {
    const { vm } = await studioApp();
    await vm.actions.support();
    assert.equal(vm.state.messages['code-result'].text, 'Diagnostics saved.');
  });

  it('reports a save the server did not confirm as a failure', async () => {
    const { vm } = await studioApp({ answers: { saveRecording: async () => undefined } });
    vm.typeField('name', 'lookup');
    await vm.actions.save();
    assert.deepEqual(vm.state.messages['save-result'], { text: 'The server did not confirm the save', error: true });
    assert.equal(vm.state.saving, false);
  });

  it('confirms a save that worked, and keeps the confirmation after the studio refreshes', async () => {
    const { vm, fake } = await studioApp({ answers: { saveRecording: async (name: string) => ({ name }) } });
    vm.focusField('name');
    vm.typeField('name', 'lookup');
    await vm.actions.save();
    assert.deepEqual(fake.called('saveRecording')[0], ['lookup', 'lookup']);
    assert.equal(vm.state.messages['save-result'].text, 'Saved “lookup” to Oya.');
  });

  it('does not save while Save is off', async () => {
    const { vm, fake } = await studioApp();
    vm.typeField('name', 'not valid');
    await vm.actions.save();
    assert.equal(fake.called('saveRecording').length, 0);
  });

  it('names the Expand control after the panel’s real width, even after a drag, and toggles it', async () => {
    const { vm, fake } = await studioApp();
    assert.equal(expandLabel(vm.state.panelWidth), 'Expand workspace');
    await vm.actions.expand();
    fake.emit('onShellLayout', { panelWidth: C.PANEL_MAX_WIDTH, panelHeight: 260, reveal: C.PANEL_MAX_WIDTH });
    assert.equal(expandLabel(vm.state.panelWidth), 'Compact workspace');
    await vm.actions.expand();
    assert.deepEqual(fake.called('resizeDevPanel'), [[C.PANEL_MAX_WIDTH], [C.PANEL_COMPACT_WIDTH]]);
  });
});
