/**
 * Unit tests for the agent panel (renderer/panes/panel.js): the Inspect tab
 * reopens its last sub-pane, and opening the panel shows the pane it was left
 * on. The narrow layout keeps panes as columns, so their lists scroll.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

describe('the agent panel', () => {
  it('reopens the Inspect sub-pane last shown when Inspect is chosen again', async () => {
    const app = loadRenderer();
    await settle();
    app.run("DevPanel.show('source')");
    app.run("DevPanel.show('chat')");
    app.$('inspect-tab').click();
    assert.equal(app.run('ShellState.activeDevPane'), 'source');
  });

  it('opens on the pane it was left on, not always Ask', async () => {
    const app = loadRenderer();
    await settle();
    app.run("DevPanel.show('network')");
    await app.run('DevPanel.toggle()');
    assert.equal(app.run('ShellState.activeDevPane'), 'network');
  });

  it('keeps every pane a scrolling column in the narrow layout', () => {
    const css = fs.readFileSync(path.join(__dirname, '../../../../renderer/shell.css'), 'utf8');
    const narrow = css.slice(css.lastIndexOf('@media (max-width: 959px)'));
    assert.match(narrow, /\.dev-pane\.active\s*\{[^}]*display:\s*flex/);
  });
});
