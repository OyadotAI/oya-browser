/**
 * Unit tests for the toolbar's control guard (renderer/control.js): while the
 * agent drives, page actions are refused, except Start recording, which takes
 * control and then records.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../support/renderer-harness.cjs');

/** The agent holds the browser and a person may take it. */
const AGENT = { mode: 'agent', interactive: false, supported: true, connected: true };
/** The person holds it. */
const MINE = { mode: 'human', mine: true, interactive: true, supported: true, connected: true };

describe('the control guard', () => {
  it('takes control, then starts recording, when Start recording is pressed under agent control', async () => {
    const app = loadRenderer({ answers: { getControlState: AGENT, changeControl: { state: MINE } } });
    await settle();
    assert.equal(app.$('record-toggle').hasAttribute('data-control-blocked'), false, 'it does not look disabled');
    app.$('record-toggle').click();
    await settle();
    assert.deepEqual(app.bridge.called('changeControl'), [['acquire']]);
    assert.equal(app.bridge.called('startRecording').length, 1);
  });

  it('does not record when taking control fails', async () => {
    const refused = { error: 'Another operator has control', state: AGENT };
    const app = loadRenderer({ answers: { getControlState: AGENT, changeControl: refused } });
    await settle();
    app.$('record-toggle').click();
    await settle();
    assert.equal(app.bridge.called('startRecording').length, 0);
    assert.equal(app.$('control-label').textContent, 'Another operator has control');
  });

  it('still refuses other page actions while watch-only', async () => {
    const app = loadRenderer({ answers: { getControlState: AGENT } });
    await settle();
    app.$('btn-reload').click();
    await settle();
    assert.equal(app.bridge.called('reload').length, 0);
    assert.equal(app.bridge.called('changeControl').length, 0);
    assert.equal(app.$('btn-reload').hasAttribute('data-control-blocked'), true);
  });
});
