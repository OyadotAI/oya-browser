/**
 * Unit tests for the Inspect Activity pane (renderer/panes/net-log.js): an
 * entry without a type is safe, rows open through one listener, selecting
 * text keeps a row open, and the pane says when it is empty.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

describe('the Activity pane', () => {
  it('shows and filters a message that has no type', async () => {
    const app = loadRenderer();
    await settle();
    app.bridge.emit('DevLog', { ts: 0, dir: 'in', data: '{}' });
    app.run("NetLog.setFilter('cmd')");
    app.run("NetLog.setFilter('all')");
    assert.equal(app.$('net-log').children.length, 1);
    assert.match(app.$('net-log').textContent, /message/);
  });

  it('opens rows through one listener on the log', async () => {
    const app = loadRenderer();
    await settle();
    for (let i = 0; i < 3; i++) app.bridge.emit('DevLog', { ts: 0, dir: 'out', type: 'cmd: click', data: '' });
    const row = app.$('net-log').children[1];
    app.fire(row, 'click', { bubbles: true });
    assert.ok(row.classList.contains('expanded'));
  });

  it('keeps a row open when the click only ended a text selection', async () => {
    let selection = '';
    const app = loadRenderer({ selection: () => selection });
    await settle();
    app.bridge.emit('DevLog', { ts: 0, dir: 'in', type: 'auth', data: 'long body' });
    const row = app.$('net-log').children[0];
    app.fire(row, 'click', { bubbles: true });
    selection = 'long';
    app.fire(row, 'click', { bubbles: true });
    assert.ok(row.classList.contains('expanded'));
  });

  it('says what will show up while the log is empty, and again after Clear', async () => {
    const app = loadRenderer();
    await settle();
    assert.equal(app.$('net-empty').hidden, false);
    app.bridge.emit('DevLog', { ts: 0, dir: 'in', type: 'auth', data: '' });
    assert.equal(app.$('net-empty').hidden, true);
    app.run('NetLog.clear()');
    assert.equal(app.$('net-empty').hidden, false);
  });

  it('names the direction in words', async () => {
    const app = loadRenderer();
    await settle();
    app.bridge.emit('DevLog', { ts: 0, dir: 'out', type: 'result: ok', data: '' });
    assert.match(app.$('net-log').textContent, /To server/);
  });
});
