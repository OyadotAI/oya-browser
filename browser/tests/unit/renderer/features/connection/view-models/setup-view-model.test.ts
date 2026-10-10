/**
 * Unit tests for the welcome screen: which server addresses the manual form
 * accepts, that a refused one is never saved, the wait for the connection,
 * sign-in through the dashboard, and a log-out leaving no key behind.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { setupProblem } from '../../../../../../src/renderer/features/connection/view-models/setup-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { build, settle } from '../harness.ts';

/** The welcome screen with the manual form filled in. */
async function filled(server: string, answers: Record<string, unknown> = {}) {
  const app = build(answers);
  await settle();
  app.setup.edit('server', server);
  app.setup.edit('apiKey', 'oya_key');
  return app;
}

describe('the manual setup form', () => {
  it('shows a terminal session failure immediately', async () => {
    const app = await filled('wss://oya.test');
    app.fake.emit('onWsStatus', { connected: false, failure: 'Restart Oya to retry.' });
    assert.equal(app.setup.state.busy, false);
    assert.equal(app.setup.state.error, 'Restart Oya to retry.');
  });
  it('accepts wss:// to any host', () => {
    assert.equal(setupProblem('wss://oya.example.com/ws', 'oya_key'), '');
  });

  it('accepts plaintext ws:// to this machine', () => {
    for (const server of ['ws://localhost:8080/ws', 'ws://127.0.0.1:8080', 'ws://[::1]:8080/ws', 'ws://LOCALHOST/ws'])
      assert.equal(setupProblem(server, 'oya_key'), '', server);
  });

  it('refuses plaintext ws:// to any other host', () => {
    for (const server of [
      'ws://oya.example.com/ws',
      'ws://10.0.0.5:8080/ws',
      'ws://localhost.evil.test/ws',
      'ws://localhost@evil.test/ws',
      'ws://localhost:80@evil.test',
    ])
      assert.match(setupProblem(server, 'oya_key'), /wss:\/\//, server);
  });

  it('requires an API key', () => {
    assert.equal(setupProblem('wss://oya.example.com/ws', ''), 'API key is required');
  });

  it('does not save a refused address', async () => {
    const app = await filled('ws://oya.example.com/ws');
    await app.setup.connect();
    assert.equal(app.fake.called('saveConfig').length, 0);
    assert.match(app.setup.state.error, /only for this computer/);
  });

  it('fills the form from the saved settings', async () => {
    const app = build({ getConfig: { serverUrl: 'wss://a.test/ws', apiKey: 'k', browserName: 'Desk' } });
    await settle();
    assert.deepEqual(
      [app.setup.state.server, app.setup.state.apiKey, app.setup.state.name],
      ['wss://a.test/ws', 'k', 'Desk'],
    );
  });
});

describe('connecting', () => {
  it('saves the trimmed settings and waits, busy', async () => {
    const app = await filled(' wss://oya.example.com/ws ');
    await app.setup.connect();
    assert.deepEqual(app.fake.called('saveConfig'), [
      [{ serverUrl: 'wss://oya.example.com/ws', apiKey: 'oya_key', browserName: undefined }],
    ]);
    assert.equal(app.setup.state.busy, true);
    app.setup.dispose();
  });

  it('says it could not connect when no connection came in time', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    try {
      const app = await filled('wss://oya.example.com/ws');
      await app.setup.connect();
      mock.timers.tick(C.CONNECT_TIMEOUT_MS);
      assert.equal(app.setup.state.busy, false);
      assert.equal(app.setup.state.error, 'Could not connect, check URL and API key');
    } finally {
      mock.timers.reset();
    }
  });

  it('stops waiting and clears the error once connected', async () => {
    const app = await filled('wss://oya.example.com/ws');
    await app.setup.connect();
    app.fake.emit('onWsStatus', { connected: true });
    assert.deepEqual([app.setup.state.busy, app.setup.state.error], [false, '']);
    app.setup.dispose();
  });

  it('says why the settings could not be saved, and gives the button back', async () => {
    const app = await filled('wss://oya.example.com/ws', {
      saveConfig: () => Promise.reject(new Error('disk full')),
    });
    await app.setup.connect();
    assert.deepEqual([app.setup.state.error, app.setup.state.busy], ['disk full', false]);
  });
});

describe('signing in with Oya', () => {
  it('opens the dashboard and waits for the pairing link', async () => {
    const app = build({ openConsole: 'https://oyabrowser.com/connect' });
    await app.setup.signIn();
    assert.equal(app.setup.state.view, 'waiting');
  });

  it('says the server address is not valid when the dashboard cannot open', async () => {
    const app = build({ openConsole: null });
    await app.setup.signIn();
    assert.equal(app.setup.state.view, 'start');
    assert.match(app.setup.state.error, /not valid/);
  });

  it('enters browsing on "Just browse for now"', () => {
    const app = build();
    app.setup.skip();
    assert.equal(app.fake.called('enterBrowsing').length, 1);
  });
});

describe('logging out', () => {
  it('goes back to the start, with nothing of the old key left in the form', async () => {
    const app = await filled('wss://oya.example.com/ws');
    app.setup.show('manual');
    app.fake.emit('onModeChanged', 'setup');
    assert.deepEqual([app.setup.state.view, app.setup.state.apiKey], ['start', '']);
  });
});
