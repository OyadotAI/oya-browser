/**
 * Unit tests for the account page of the shell dialog
 * (renderer/connection/profile.js): who this browser is signed in as, the
 * connection, when its logins last synced, the profile it browses as, the
 * device, and times in words.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

const DEVICE = { id: 'p-1', platform: 'Win32', timezone: 'Europe/London', locale: 'en-GB', screen: '1920x1080' };
const ACCOUNT = { email: 'ada@example.com', name: 'Ada Lovelace', plan: 'pro', project: { id: 'prj_1', name: 'Lab' } };
const CONFIG = { serverUrl: 'wss://oyabrowser.com/ws', apiKey: 'k', browserName: 'Desk' };
const MINUTE = 60_000;

/** The shell with the account page open; `answers` override the bridge's. */
async function accountPage(answers = {}) {
  const app = loadRenderer({ answers: { getConfig: CONFIG, getAccount: ACCOUNT, ...answers } });
  await settle();
  await app.run('ShellDialog.open(true)');
  await settle();
  return app;
}

describe('the account card', () => {
  it('names the person, their email, project and plan, with their initial', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    assert.equal(app.$('account-name').textContent, 'Ada Lovelace');
    assert.deepEqual([app.$('account-email').textContent, app.$('account-email').hidden], ['ada@example.com', false]);
    assert.equal(app.$('account-meta').textContent, 'Project Labpro plan');
    assert.equal(app.$('account-avatar').textContent, 'A');
    assert.equal(app.$('sign-out').textContent, 'Log out of ada@example.com');
  });

  it('says a key alone signs the browser in when no account is behind it, with the project', async () => {
    const app = await accountPage({ getAccount: { email: null, project: { id: 'prj_2', name: 'robots' } } });
    assert.equal(app.$('account-name').textContent, 'Signed in with an API key');
    assert.equal(app.$('account-email').hidden, true);
    assert.equal(app.$('account-meta').textContent, 'Project robots');
    assert.equal(app.$('account-avatar').textContent, 'R');
    assert.equal(app.$('sign-out').textContent, 'Log out');
  });

  it('shows the connection: connected, reconnecting with a key, offline without one', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    assert.equal(app.$('account-status-text').textContent, 'Connected to oyabrowser.com');
    app.bridge.emit('WsStatus', { connected: false });
    assert.equal(app.$('account-status').dataset.state, 'reconnecting');
    assert.equal(app.$('account-status-text').textContent, 'Reconnecting to oyabrowser.com…');
    const offline = await accountPage({ getConfig: { ...CONFIG, apiKey: '' }, getAccount: null });
    assert.equal(offline.$('account-status-text').textContent, 'Offline');
  });

  it('opens Switch account on the console of the server this browser uses', async () => {
    const app = await accountPage();
    app.$('switch-account').click();
    assert.deepEqual(app.bridge.called('openConsole'), [['wss://oyabrowser.com/ws']]);
  });

  it('keeps Advanced folded until asked for', async () => {
    const app = await accountPage();
    assert.equal(app.$('profile-advanced').open, false);
    assert.equal(app.$('profile-server').textContent, 'wss://oyabrowser.com/ws');
  });
});

describe('sync', () => {
  it('says when the logins last reached the server, and how many sites it keeps', async () => {
    const lastSync = { at: Date.now() - 2 * MINUTE, sites: 937 };
    const app = await accountPage({ getConfig: { ...CONFIG, lastSync }, getStatus: { connected: true } });
    assert.equal(app.$('sync-title').textContent, 'Logins synced 2 minutes ago · 937 sites');
    assert.equal(app.$('sync-now').disabled, false);
  });

  it('takes the later of the last Sync now and the last automatic send', async () => {
    const lastSync = { at: Date.now() - 60 * MINUTE, sites: 3 };
    const status = { connected: true, syncedAt: Date.now() - 5 * MINUTE };
    const app = await accountPage({ getConfig: { ...CONFIG, lastSync }, getStatus: status });
    assert.equal(app.$('sync-title').textContent, 'Logins synced 5 minutes ago · 3 sites');
  });

  it('syncs now and says just now with the new count once the server confirms', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    app.$('sync-now').click();
    await settle();
    assert.equal(app.bridge.called('saveProfile').length, 1);
    assert.equal(app.$('sync-now').disabled, true);
    app.bridge.emit('ProfileSaved', { sites: ['a.test'] });
    assert.equal(app.$('sync-title').textContent, 'Logins synced just now · 1 site');
    assert.equal(app.$('sync-now').disabled, false);
  });

  it('says what has to happen first when offline', async () => {
    const app = await accountPage();
    assert.equal(app.$('sync-title').textContent, 'Logins sync when you reconnect');
    assert.equal(app.$('sync-now').disabled, true);
  });
});

describe('browsing as', () => {
  it('names the profile whose cookies Oya uses, in plain words', async () => {
    const app = await accountPage({ getStatus: { connected: true, profileName: 'Firefox · default' } });
    assert.equal(app.$('persona-name').textContent, '“Firefox · default”');
    assert.equal(app.$('fp-content').textContent, 'Oya signs in to sites with this profile’s cookies and logins.');
  });
});

describe('this device', () => {
  it('shows the device this browser runs as, in words', async () => {
    const app = loadRenderer({ answers: { getFingerprint: DEVICE } });
    await settle();
    assert.equal(app.$('profile-device').textContent, 'Windows · Europe/London · en-GB · 1920x1080');
  });

  it('follows a change of persona', async () => {
    const app = loadRenderer({ answers: { getFingerprint: DEVICE } });
    await settle();
    app.bridge.emit('FingerprintChanged', { ...DEVICE, platform: 'MacIntel', timezone: 'America/New_York' });
    assert.match(app.$('profile-device').textContent, /^Mac · America\/New_York/);
  });

  it('says so when no persona has been assigned yet', async () => {
    const app = loadRenderer({ answers: { getFingerprint: null } });
    await settle();
    assert.equal(app.$('profile-device').textContent, 'This computer, until you connect');
  });

  it('saves a new name as it is changed, and nothing for an empty or unchanged one', async () => {
    const app = await accountPage({ saveConfig: true });
    const input = app.$('profile-name');
    assert.equal(input.value, 'Desk');
    input.value = '  ';
    app.fire(input, 'change');
    input.value = 'Desk';
    app.fire(input, 'change');
    input.value = 'Studio Mac';
    app.fire(input, 'change');
    await settle();
    assert.equal(JSON.stringify(app.bridge.called('saveConfig')), '[[{"browserName":"Studio Mac"}]]');
    assert.equal(app.$('profile-name-status').textContent, 'Name saved.');
  });
});

describe('When', () => {
  it('tells a past time in words', async () => {
    const app = loadRenderer();
    const ago = (ms) => app.run(`When.ago(${1e12 - ms}, ${1e12})`);
    assert.deepEqual(
      [ago(10_000), ago(5 * MINUTE), ago(26 * 60 * MINUTE), ago(3 * 24 * 60 * MINUTE)],
      ['just now', '5 minutes ago', 'yesterday', '3 days ago'],
    );
  });
});
