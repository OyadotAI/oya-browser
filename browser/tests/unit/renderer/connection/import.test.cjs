/**
 * Unit tests for "Import logins" in the connection dialog
 * (renderer/connection/import.js): pick one of the browsers on this computer,
 * import it, and read plainly how it went.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

const SOURCES = [
  { id: 'firefox', name: 'Firefox', isDefault: true },
  { id: 'chrome', name: 'Chrome', isDefault: false },
];

/** The shell, connected, with `sources` installed. */
async function connected(sources = SOURCES) {
  const app = loadRenderer({ answers: { importSources: sources } });
  await settle();
  app.bridge.emit('WsStatus', { connected: true, profileName: 'Default' });
  await settle();
  return app;
}

describe('Import logins', () => {
  it("offers the browsers on this computer, the person's default first", async () => {
    const app = await connected();
    const options = app.$('import-source').querySelectorAll('option');
    assert.deepEqual(
      options.map((o) => [o.value, o.textContent]),
      [
        ['firefox', 'Firefox (your default browser)'],
        ['chrome', 'Chrome'],
      ],
    );
    assert.equal(app.$('import-logins').disabled, false);
  });

  it('imports the chosen browser and says what is happening, then what came over', async () => {
    const app = await connected();
    app.$('import-source').value = 'chrome';
    app.$('import-logins').click();
    await settle();
    assert.deepEqual(app.bridge.called('reimportBrowser'), [['chrome']]);
    app.bridge.emit('MirrorStatus', { started: true });
    assert.equal(app.$('import-status').textContent, 'Reading your logins from Chrome…');
    assert.equal(app.$('import-logins').disabled, true);
    app.bridge.emit('MirrorStatus', { done: true, source: 'Chrome', profiles: 2, cookies: 412 });
    assert.equal(
      app.$('import-status').textContent,
      'Imported Chrome: 2 profiles, 412 cookies. You are signed in here now.',
    );
    assert.equal(app.$('import-logins').disabled, false);
  });

  it('still says what was imported after the reconnect that follows an import', async () => {
    const app = await connected();
    app.bridge.emit('MirrorStatus', { done: true, source: 'Firefox', profiles: 1, cookies: 2 });
    app.bridge.emit('WsStatus', { connected: false });
    assert.equal(app.$('import-status').textContent, 'Connect to Oya to import your logins.');
    app.bridge.emit('WsStatus', { connected: true });
    assert.equal(
      app.$('import-status').textContent,
      'Imported Firefox: 1 profile, 2 cookies. You are signed in here now.',
    );
    assert.equal(app.$('import-logins').disabled, false);
  });

  it('says why an import failed, and lets it be tried again', async () => {
    const app = await connected();
    app.$('import-logins').click();
    await settle();
    app.bridge.emit('MirrorStatus', { done: true, error: 'Chrome never opened its debugging port' });
    assert.equal(app.$('import-status').textContent, 'Chrome never opened its debugging port');
    assert.equal(app.$('import-status').dataset.kind, 'error');
    assert.equal(app.$('import-logins').disabled, false);
  });

  it('says so when the browser had nothing to bring over', async () => {
    const app = await connected();
    app.bridge.emit('MirrorStatus', { done: true, empty: true });
    assert.equal(app.$('import-status').textContent, 'Nothing to import: that browser has no profiles with logins.');
  });

  it('knows it is connected when the app connected before this page was listening', async () => {
    const status = { connected: true, browserId: 'b1', profileName: 'Work' };
    const app = loadRenderer({ answers: { importSources: SOURCES, getStatus: status } });
    await settle();
    assert.equal(app.$('import-logins').disabled, false);
    assert.equal(app.$('import-status').textContent, '');
    assert.equal(app.$('fp-content').textContent, 'Profile · Work · Account sessions sync automatically');
  });

  it('waits for a connection, and says so', async () => {
    const app = loadRenderer({ answers: { importSources: SOURCES } });
    await settle();
    app.bridge.emit('WsStatus', { connected: false });
    await settle();
    assert.equal(app.$('import-logins').disabled, true);
    assert.equal(app.$('import-status').textContent, 'Connect to Oya to import your logins.');
  });

  it('says so when no supported browser is installed', async () => {
    const app = await connected([]);
    assert.equal(app.$('import-controls').hidden, true);
    assert.equal(app.$('import-status').textContent, 'No supported browser found on this computer.');
  });
});
