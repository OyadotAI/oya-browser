/**
 * Unit tests for the step editor (renderer/studio/editor.js): its heading
 * follows moves, a redraw keeps focus, quick edits build on each other, and
 * running to a step needs a runnable workflow.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { studioApp } = require('../../support/studio.cjs');
const { Event } = require('../../support/fake-dom.cjs');

/** The editor's field for `key`. */
const field = ($, key) =>
  $('step-editor')
    .querySelectorAll('[data-key]')
    .find((el) => el.dataset.key === key);

/** The editor's button labelled `label`. */
const button = ($, label) =>
  $('step-editor')
    .querySelectorAll('button')
    .find((el) => el.getAttribute('aria-label') === label);

/** Sets a field's value and fires its change event. */
function change(input, value) {
  input.value = value;
  input.dispatchEvent(new Event('change'));
}

/** Selects step `id` and redraws. */
async function select(app, id, settle) {
  app.run(`StudioSteps.select(${JSON.stringify(id)})`);
  await settle();
}

describe('the step editor', () => {
  it('renumbers its heading when the step moves', async () => {
    const { app, $, settle } = await studioApp();
    await select(app, 'b', settle);
    assert.equal($('step-editor').querySelector('h3').textContent, 'Step 2 · Click');
    button($, 'Move up').click();
    await settle();
    assert.equal($('step-editor').querySelector('h3').textContent, 'Step 1 · Click');
  });

  it('keeps focus in the field being edited when its answer redraws the editor', async () => {
    const { app, $, settle } = await studioApp();
    await select(app, 'b', settle);
    field($, 'Target').focus();
    change(field($, 'Target'), '#next');
    await settle();
    assert.equal(app.document.activeElement.dataset.key, 'Target');
    assert.ok(app.document.activeElement.closest('#step-editor'));
  });

  it('builds a second quick edit on the first instead of overwriting it', async () => {
    const { app, ws, $, settle } = await studioApp();
    await select(app, 'b', settle);
    change(field($, 'strategy'), 'text');
    change(field($, 'Target'), 'Go');
    await settle();
    const target = ws.draft.steps.find((s) => s.id === 'b').candidates[0];
    assert.deepEqual([target.kind, target.value], ['text', 'Go']);
  });

  it('turns off Run to here while a step blocks the run', async () => {
    const { app, $, settle } = await studioApp({ steps: [{ id: 'x', action: 'click', candidates: [] }] });
    await select(app, 'x', settle);
    const run = $('step-editor')
      .querySelectorAll('button')
      .find((b) => b.textContent === 'Run to here');
    assert.equal(run.disabled, true);
  });

  it('sends no timeout when the timeout field is cleared', async () => {
    const sent = [];
    const { app, ws, $, settle } = await studioApp({
      commands: { update: (cmd) => (sent.push(cmd.patch), ws.snapshot()) },
    });
    await select(app, 'b', settle);
    change(field($, 'Timeout (ms)'), '');
    await settle();
    // The patch comes from the renderer's own realm, so compare its contents, not its prototype.
    assert.equal(sent.length, 1);
    assert.ok(Object.hasOwn(sent[0], 'timeout'));
    assert.equal(sent[0].timeout, undefined);
  });
});
