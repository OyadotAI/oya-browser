/**
 * Unit tests for "Save as playbook" under an Ask reply (renderer/panes/chat-playbook.js):
 * offered only for a run with an action to replay, and its tool list kept equal
 * to the server's.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

/** A reply element with the offer made for `tools` (and the server's `canSave`); true when a Save button appeared. */
async function offered(tools, canSave) {
  const app = loadRenderer();
  await settle();
  const message = app.document.createElement('div');
  app.document.body.appendChild(message);
  app.window.message = message;
  app.window.calls = tools.map((name) => ({ name }));
  app.window.canSave = canSave;
  app.run("ChatPlaybook.offer(message, 'check my inbox', calls, canSave)");
  return !!message.querySelector('.chat-save-button');
}

/** The quoted tool names in the Set literal that follows `name` in `file`. */
function toolSet(file, name) {
  const text = fs.readFileSync(path.join(__dirname, file), 'utf8');
  const body = text.slice(text.indexOf(name)).match(/new Set\(\[([^\]]*)\]/)[1];
  return [...body.matchAll(/'([a-z_]+)'/g)].map((m) => m[1]).sort();
}

describe('Save as playbook', () => {
  it('is not offered for a run that only visited and read pages', async () => {
    assert.equal(await offered(['navigate', 'analyze_page', 'navigate', 'analyze_page']), false);
  });

  it('is offered once the run acted on a page', async () => {
    assert.equal(await offered(['navigate', 'analyze_page', 'click']), true);
    assert.equal(await offered(['switch_tab', 'keyboard_type']), true);
  });

  it('follows the server when it says whether the run can be saved', async () => {
    assert.equal(await offered(['navigate', 'wait', 'click'], false), false, 'refused calls recorded nothing');
    assert.equal(await offered(['navigate'], true), true);
  });

  it('knows exactly the tools the server records', () => {
    const renderer = toolSet('../../../../renderer/panes/chat-playbook.js', 'REPLAYABLE_TOOLS');
    const server = toolSet('../../../../../server/src/modules/agent/recorder.ts', 'const RECORDED');
    assert.deepEqual(renderer, server);
  });
});
