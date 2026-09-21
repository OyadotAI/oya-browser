/**
 * Unit tests for the studio's main view (renderer/studio/view.js): labels,
 * the save card, what disables which control, and where messages show.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { studioApp, STEPS } = require('../../support/studio.cjs');
const { Event } = require('../../support/fake-dom.cjs');

/** Types `value` into an input and fires its input event. */
function type(input, value) {
  input.value = value;
  input.dispatchEvent(new Event('input'));
}

describe('the workflow studio', () => {
  it('says "Saved" only while the draft is what was saved, and "Save changes" after an edit', async () => {
    const { ws, push, $ } = await studioApp();
    ws.edit({ type: 'metadata', name: 'lookup' });
    Object.assign(ws.draft, { publishedAt: 1, publishedRevision: ws.draft.revision });
    await push();
    assert.equal($('record-save').textContent, 'Saved');
    assert.equal($('record-save').disabled, true);
    ws.edit({ type: 'metadata', name: 'lookup-2' });
    await push();
    assert.equal($('record-save').textContent, 'Save changes');
    assert.equal($('record-save').disabled, false);
  });

  it('counts steps in words, with one step singular', async () => {
    const { $ } = await studioApp({ steps: STEPS.slice(0, 1) });
    assert.equal($('record-count').textContent, '1 step');
  });

  it('says a recording was just captured only right after one, never for a draft opened later', async () => {
    let stop;
    const { ws, push, $, settle } = await studioApp({ answers: { stopRecording: () => stop() } });
    assert.equal($('record-finish-title').textContent, 'Save to Oya');
    ws.capture(ws.draft.steps, [], true);
    await push();
    stop = async () => ws.capture(ws.draft.steps, [], false);
    $('record-toggle').click();
    await settle();
    assert.equal($('record-finish-title').textContent, 'Recording captured');
    $('record-clear').click();
    await settle();
    ws.capture(structuredClone(STEPS), [], false);
    await push();
    assert.equal($('record-finish-title').textContent, 'Save to Oya');
  });

  it('never says "Saving…" while a recording is being stopped', async () => {
    let release;
    const answers = { stopRecording: () => new Promise((resolve) => (release = resolve)) };
    const { ws, push, $, settle } = await studioApp({ answers });
    ws.capture(ws.draft.steps, [], true);
    await push();
    $('record-toggle').click();
    await settle();
    assert.notEqual($('record-save').textContent, 'Saving…');
    ws.capture(ws.draft.steps, [], false);
    release();
    await settle();
  });

  it('keeps the error when resuming a recording fails', async () => {
    const commands = {
      'resume-recording': () => {
        throw new Error("Error invoking remote method 'workspace': Error: Take control first");
      },
    };
    const { $, settle } = await studioApp({ commands });
    $('record-toggle').click();
    await settle();
    assert.equal($('record-result').textContent, 'Take control first');
    assert.ok($('record-result').classList.contains('error'));
  });

  it('lists what blocks a test run with no step to point at, under Test run', async () => {
    const { ws, app, $, settle } = await studioApp();
    app.bridge.emit('Workspace', { ...ws.snapshot(), issues: [{ message: 'Start with a page to open' }] });
    await settle();
    assert.equal($('studio-issues').textContent, 'Start with a page to open');
    assert.equal($('record-validate').disabled, true);
  });

  it('keeps Test run and Save off until the workspace has answered', async () => {
    const { $ } = await studioApp({ commands: { get: () => undefined } });
    assert.equal($('record-validate').disabled, true);
    assert.equal($('record-save').disabled, true);
  });

  it('turns off New and the workflow list while the draft cannot be stored, and says why', async () => {
    const { $ } = await studioApp({ storage: { failSave: true } });
    assert.equal($('record-clear').disabled, true);
    assert.equal($('draft-library').disabled, true);
    assert.equal($('draft-status').hidden, false);
    assert.match($('record-finish-copy').textContent, /only in memory/);
  });

  it('turns Save off and says why as soon as the name cannot be published', async () => {
    const { $ } = await studioApp();
    type($('record-name'), 'my workflow');
    assert.equal($('record-save').disabled, true);
    assert.match($('record-save-hint').textContent, /letters, numbers/);
    type($('record-name'), 'my-workflow');
    assert.equal($('record-save').disabled, false);
  });

  it('locks the name, description and variables while recording', async () => {
    const { ws, push, $ } = await studioApp();
    ws.capture(ws.draft.steps, [], true);
    await push();
    for (const id of ['record-name', 'record-desc', 'variable-add', 'step-add', 'record-clear']) {
      assert.equal($(id).disabled, true, id);
    }
    assert.equal($('pane-record').dataset.stage, 'recording');
  });

  it('clears the last draft’s messages when another draft is opened', async () => {
    const { ws, push, $, studio } = await studioApp();
    studio().say('Saved “x” to Oya.', false, 'save-result');
    ws.edit({ type: 'new' });
    await push();
    assert.equal($('save-result').textContent, '');
  });
});
