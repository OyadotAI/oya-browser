/**
 * Unit tests for the connection settings dialog: opened from the account
 * page, filled with the saved settings, saving and waiting for the
 * connection, retrying, and closing once connected.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { build, settle } from '../harness.ts';

const CONFIG = { serverUrl: 'wss://oyabrowser.com/ws', apiKey: 'k', browserName: 'Desk' };

/** The dialog opened from the account page. */
async function opened(answers: Record<string, unknown> = {}) {
  const app = build({ getConfig: CONFIG, saveConfig: true, ...answers });
  await settle();
  await app.dialog.open(true);
  await app.reconnect.editSettings();
  return app;
}

describe('the reconnect dialog', () => {
  it('closes the account dialog and opens filled with the saved settings', async () => {
    const app = await opened();
    assert.equal(app.dialog.state.open, false);
    const s = app.reconnect.state;
    assert.deepEqual(
      [s.open, s.server, s.apiKey, s.name, s.button],
      [true, CONFIG.serverUrl, 'k', 'Desk', 'Save & Reconnect'],
    );
    assert.deepEqual(app.fake.called('showOverlay').at(-1), ['legacy']);
  });

  it('refuses an address that is not ws:// or wss://, or no key', async () => {
    const app = await opened();
    app.reconnect.edit('server', 'http://oya.test');
    await app.reconnect.save();
    assert.equal(app.reconnect.state.error, 'Enter a ws:// or wss:// server address and an API key.');
    assert.equal(app.fake.called('saveConfig').length, 0);
  });

  it('saves, shows the new settings on the account page, and waits for the connection', async () => {
    const app = await opened();
    app.reconnect.edit('server', 'wss://other.test/ws');
    app.reconnect.edit('name', 'Laptop');
    await app.reconnect.save();
    assert.deepEqual(app.fake.called('saveConfig').at(-1), [
      { serverUrl: 'wss://other.test/ws', apiKey: 'k', browserName: 'Laptop' },
    ]);
    assert.deepEqual([app.reconnect.state.saving, app.reconnect.state.button], [true, 'Connecting…']);
    assert.equal(app.dialog.state.server, 'wss://other.test/ws');
    assert.equal(app.profile.state.name, 'Laptop');
    app.reconnect.dispose();
  });

  it('offers a retry when no connection came in time', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    try {
      const app = await opened();
      await app.reconnect.save();
      mock.timers.tick(C.CONNECT_TIMEOUT_MS);
      assert.deepEqual([app.reconnect.state.saving, app.reconnect.state.button], [false, 'Retry connection']);
      assert.match(app.reconnect.state.error, /Could not connect/);
    } finally {
      mock.timers.reset();
    }
  });

  it('says why saving failed and offers a retry', async () => {
    const app = await opened({ saveConfig: () => Promise.reject(new Error('locked')) });
    await app.reconnect.save();
    assert.deepEqual([app.reconnect.state.error, app.reconnect.state.button], ['locked', 'Retry connection']);
  });

  it('closes, giving the page back, once connected', async () => {
    const app = await opened();
    await app.reconnect.save();
    app.fake.emit('onWsStatus', { connected: true });
    assert.equal(app.reconnect.state.open, false);
    assert.equal(app.reconnect.state.button, 'Save & Reconnect');
    assert.deepEqual(app.fake.called('hideOverlay').at(-1), ['legacy']);
  });

  it('gives the page back and says so when the settings cannot be read', async () => {
    const app = build({ getConfig: () => Promise.reject(new Error('unreadable')) });
    await app.reconnect.open();
    assert.equal(app.reconnect.state.open, false);
    assert.deepEqual(app.fake.called('hideOverlay'), [['legacy']]);
    assert.equal(app.setup.state.error, 'unreadable');
  });
});
