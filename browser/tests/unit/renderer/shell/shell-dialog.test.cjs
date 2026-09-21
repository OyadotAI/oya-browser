/**
 * Unit tests for the connection and profile dialog: it shows the server the
 * browser is on now, not the one it was on when the window loaded.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

describe('the connection and profile dialog', () => {
  it('shows the server saved since start, after a pairing link retargets the browser', async () => {
    const config = { serverUrl: 'ws://localhost:3100/ws', browserName: 'Mine' };
    const app = loadRenderer({ answers: { getConfig: () => config } });
    await settle();
    config.serverUrl = 'wss://oyabrowser.com/ws';
    await app.run('ShellDialog.open(true)');
    await settle();
    assert.equal(app.$('profile-server').textContent, 'wss://oyabrowser.com/ws');
  });
});
