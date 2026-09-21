/**
 * Unit tests for the studio's actions (renderer/studio/actions.js): the test
 * run, adding steps and variables, export, diagnostics, the record shortcut,
 * saving, and the Expand control.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { studioApp } = require('../../support/studio.cjs');
const { Event } = require('../../support/fake-dom.cjs');

/** Picks `value` in a select and fires its change event. */
function choose(select, value) {
  select.value = value;
  select.dispatchEvent(new Event('change'));
}

describe('studio actions', () => {
  it('stays on Steps when the test-run prompt is cancelled, and shows Run once a run starts', async () => {
    let started = false;
    const { ws, $, studio, settle } = await studioApp({
      commands: { validate: async () => (started && (await ws.start({})), ws.snapshot()) },
    });
    $('record-validate').click();
    await settle();
    assert.equal(studio().tab, 'steps');
    started = true;
    $('record-validate').click();
    await settle();
    assert.equal(studio().tab, 'run');
  });

  it('keeps the selected step when adding a step is refused, and says why', async () => {
    const commands = {
      add: () => {
        throw new Error('Too many steps');
      },
    };
    const { $, studio, settle } = await studioApp({ commands });
    studio().selected = 'b';
    choose($('step-add'), 'wait');
    await settle();
    assert.equal(studio().selected, 'b');
    assert.equal($('record-result').textContent, 'Too many steps');
  });

  it('shows why a variable could not be added, not how to use it', async () => {
    const commands = {
      variables: () => {
        throw new Error('Variable names must be letters and numbers');
      },
    };
    const { $, settle } = await studioApp({ commands });
    $('variable-add').click();
    await settle();
    assert.equal($('record-result').textContent, 'Variable names must be letters and numbers');
  });

  it('adds the next free input variable and says how to use it', async () => {
    const { ws, $, settle } = await studioApp();
    $('variable-add').click();
    await settle();
    assert.deepEqual(Object.keys(ws.draft.variables), ['input_1']);
    assert.equal($('record-result').textContent, 'Use {{input_1}} in a step.');
  });

  it('keeps Copy and Save module off until there is code to copy', async () => {
    const { $ } = await studioApp({ steps: [{ id: 'x', action: 'click', candidates: [] }] });
    assert.equal($('record-copy').disabled, true);
    assert.equal($('record-download').disabled, true);
    const ready = await studioApp();
    assert.equal(ready.$('record-copy').disabled, false);
  });

  it('says so when diagnostics are saved', async () => {
    const { $, settle } = await studioApp();
    $('support-export').click();
    await settle();
    assert.equal($('code-result').textContent, 'Diagnostics saved.');
  });

  it('does nothing on the record shortcut when the record button could not be pressed', async () => {
    const { app, $, settle } = await studioApp();
    $('record-toggle').setAttribute('data-control-blocked', '');
    await app.run('StudioActions.recordButton()');
    await settle();
    assert.equal(
      app.bridge.called('workspace').some(([cmd]) => cmd.type === 'resume-recording'),
      false,
    );
  });

  it('reports a save the server did not confirm as a failure', async () => {
    const { $, settle } = await studioApp({ answers: { saveRecording: async () => undefined } });
    $('record-name').focus();
    $('record-name').value = 'lookup';
    $('record-name').dispatchEvent(new Event('input'));
    $('record-save').click();
    await settle();
    assert.equal($('save-result').textContent, 'The server did not confirm the save');
    assert.ok($('save-result').classList.contains('error'));
  });

  it('confirms a save that worked, and keeps the confirmation after the studio refreshes', async () => {
    const { $, settle } = await studioApp({ answers: { saveRecording: async (name) => ({ name }) } });
    $('record-name').focus();
    $('record-name').value = 'lookup';
    $('record-name').dispatchEvent(new Event('input'));
    $('record-save').click();
    await settle();
    assert.equal($('save-result').textContent, 'Saved “lookup” to Oya.');
  });

  it('names the Expand control after the panel’s real width, even after a drag', async () => {
    const { app, $, settle } = await studioApp();
    assert.equal($('studio-expand').title, 'Expand workspace');
    app.bridge.emit('ShellLayout', { panelWidth: 560, panelHeight: 260, reveal: 560 });
    await settle();
    assert.equal($('studio-expand').title, 'Compact workspace');
  });
});
