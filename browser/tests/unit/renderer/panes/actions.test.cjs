/**
 * Unit tests for the Inspect Actions pane (renderer/panes/actions.js): inputs
 * are checked before sending, one action runs at a time with a labelled
 * result, page actions wait for control, and an analysis lists elements.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

/** The pane's button for `action`. */
const button = (app, action) => app.document.querySelector(`#pane-actions [data-action="${action}"]`);

/** The dev actions the pane sent, as [action, params]. */
const sent = (app) => app.bridge.called('devAction');

describe('the Actions pane', () => {
  it('refuses an empty or non-numeric coordinate instead of clicking the corner', async () => {
    const app = loadRenderer();
    await settle();
    app.$('action-cx').value = '';
    app.$('action-cy').value = '5';
    button(app, 'click-coords').click();
    await settle();
    assert.deepEqual(sent(app), []);
    assert.match(app.$('action-result-body').textContent, /X and Y as numbers/);
  });

  it('refuses an element number that is not a whole number', async () => {
    const app = loadRenderer();
    await settle();
    app.$('action-click-id').value = 'a1b2';
    button(app, 'click').click();
    await settle();
    assert.deepEqual(sent(app), []);
    assert.match(app.$('action-result-body').textContent, /element number/);
  });

  it('runs a row’s action when Enter is pressed in its field', async () => {
    const app = loadRenderer({ answers: { devAction: { ok: true, data: {} } } });
    await settle();
    app.$('action-key').value = 'Escape';
    app.key(app.$('action-key'), 'Enter');
    await settle();
    assert.deepEqual(sent(app)[0][0], 'press-key');
  });

  it('turns off page actions while the agent has control, but not a screenshot', async () => {
    const app = loadRenderer();
    await settle();
    app.bridge.emit('ControlState', { interactive: false, mode: 'agent' });
    assert.equal(button(app, 'click').disabled, true);
    assert.equal(button(app, 'analyze').disabled, true);
    assert.equal(button(app, 'screenshot').disabled, false);
  });

  it('runs one action at a time and labels the result with it', async () => {
    let finish;
    const app = loadRenderer({ answers: { devAction: () => new Promise((resolve) => (finish = resolve)) } });
    await settle();
    button(app, 'reload').click();
    button(app, 'screenshot').click();
    await settle();
    assert.equal(sent(app).length, 1);
    assert.equal(button(app, 'screenshot').disabled, true);
    finish({ ok: true });
    await settle();
    assert.match(app.$('action-result-title').textContent, /^Reload · done/);
    assert.equal(button(app, 'screenshot').disabled, false);
  });

  it('says when a long result was cut short', async () => {
    const page = 'x'.repeat(app_limit() + 10);
    const app = loadRenderer({ answers: { devAction: { ok: true, data: { page, elements: [] } } } });
    await settle();
    button(app, 'analyze').click();
    await settle();
    assert.match(app.$('action-result-body').textContent, /Showing the first part/);
  });

  it('lists an analysis’s elements, and picking one fills the element number', async () => {
    const elements = [{ id: 12, type: 'button', text: 'Sign in', visible: true }];
    const app = loadRenderer({ answers: { devAction: { ok: true, data: { page: '# A', elements } } } });
    await settle();
    button(app, 'analyze').click();
    await settle();
    const item = app.$('action-elements').children[0];
    assert.equal(item.textContent, '#12 button Sign in');
    item.click();
    assert.equal(app.$('action-click-id').value, '12');
  });

  it('gives New tab a plus icon and every field a real name', async () => {
    const app = loadRenderer();
    await settle();
    assert.match(button(app, 'new-tab').innerHTML, /M12 5v14/);
    for (const field of app.document.querySelectorAll('#pane-actions .action-field')) {
      assert.ok(field.getAttribute('aria-label').length > 2, field.id);
    }
  });
});

/** The analyze preview's length, read from the renderer's constants. */
function app_limit() {
  const app = loadRenderer();
  return app.run('RendererConstants.ANALYZE_PREVIEW');
}
