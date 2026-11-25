/**
 * Unit tests for the account card: who this browser is signed in as, the
 * project and plan, the connection, Log out's label, Switch account, and
 * learning the account once the server can answer.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  accountCard,
  hostOf,
} from '../../../../../../src/renderer/features/connection/view-models/account-view-model.ts';
import { build, settle } from '../harness.ts';

const ACCOUNT = { email: 'ada@example.com', name: 'Ada Lovelace', plan: 'pro', project: { id: 'prj_1', name: 'Lab' } };
const CONFIG = { serverUrl: 'wss://oyabrowser.com/ws', apiKey: 'k', browserName: 'Desk' };

/** The account page opened, over `answers`. */
async function accountPage(answers: Record<string, unknown> = {}) {
  const app = build({ getConfig: CONFIG, getAccount: ACCOUNT, ...answers });
  await settle();
  await app.dialog.open(true);
  return app;
}

describe('the account card', () => {
  it('shows terminal failure instead of promising endless reconnection', async () => {
    const failure = 'Session setup failed. Restart Oya to retry.';
    const app = await accountPage({ getStatus: { connected: false, failure } });
    assert.equal(accountCard(app.account.state).connection, 'offline');
    assert.equal(accountCard(app.account.state).connectionText, failure);
    app.account.onStatus({ connected: true });
    assert.equal(app.account.state.failure, undefined);
  });
  it('does not say "Project" twice for a project whose name already starts with it', () => {
    const card = accountCard({ account: { project: { name: 'Project 1b9258' } }, connected: true, config: {} } as any);
    assert.equal(card.project, 'Project 1b9258');
    assert.equal(
      accountCard({ account: { project: { name: 'Acme' } }, connected: true, config: {} } as any).project,
      'Project Acme',
    );
  });

  it('names the person, their email, project and plan, with their initial', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    const card = accountCard(app.account.state);
    assert.deepEqual(
      [card.title, card.email, card.project, card.plan, card.initial],
      ['Ada Lovelace', 'ada@example.com', 'Project Lab', 'pro plan', 'A'],
    );
    assert.equal(card.signOut.text, 'Log out of ada@example.com');
  });

  it('says a key alone signs the browser in when no account is behind it, with the project', async () => {
    const app = await accountPage({ getAccount: { email: null, project: { id: 'prj_2', name: 'robots' } } });
    const card = accountCard(app.account.state);
    assert.deepEqual(
      [card.title, card.email, card.project, card.initial],
      ['Signed in with an API key', '', 'Project robots', 'R'],
    );
    assert.equal(card.signOut.text, 'Log out');
  });

  it('asks to connect to see the project when offline and unknown', async () => {
    const app = await accountPage({ getAccount: null });
    assert.equal(accountCard(app.account.state).project, 'Connect to see your project');
  });

  it('shows the connection: connected, reconnecting with a key, offline without one', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    assert.equal(accountCard(app.account.state).connectionText, 'Connected to oyabrowser.com');
    app.fake.emit('onWsStatus', { connected: false });
    const card = accountCard(app.account.state);
    assert.deepEqual([card.connection, card.connectionText], ['reconnecting', 'Reconnecting to oyabrowser.com…']);
    const offline = await accountPage({ getConfig: { ...CONFIG, apiKey: '' }, getAccount: null });
    assert.equal(accountCard(offline.account.state).connectionText, 'Offline');
  });

  it('names an unreadable server plainly', () => {
    assert.equal(hostOf('not a url'), 'the server');
  });

  it('opens Switch account on the console of the server this browser uses', async () => {
    const app = await accountPage();
    app.account.switchAccount();
    assert.deepEqual(app.fake.called('openConsole'), [['wss://oyabrowser.com/ws']]);
  });

  it('signs out', async () => {
    const app = await accountPage();
    app.account.signOut();
    assert.equal(app.fake.called('signOut').length, 1);
  });

  it('learns the account once connected while the page is open', async () => {
    let account: unknown = null;
    const app = await accountPage({ getAccount: () => account });
    account = ACCOUNT;
    app.fake.emit('onWsStatus', { connected: true });
    await settle();
    assert.equal(accountCard(app.account.state).title, 'Ada Lovelace');
  });

  it('asks again on a new connection, so a pairing as someone else shows them', async () => {
    let account: unknown = ACCOUNT;
    const app = await accountPage({ getAccount: () => account });
    account = { ...ACCOUNT, name: '', email: 'grace@example.com' };
    app.fake.emit('onWsStatus', { connected: false });
    app.fake.emit('onWsStatus', { connected: true });
    await settle();
    assert.equal(accountCard(app.account.state).title, 'grace@example.com');
  });

  it('learns the account on connecting while the page is closed, for the toolbar avatar and profile menu', async () => {
    let account: unknown = null;
    const app = build({ getConfig: CONFIG, getAccount: () => account });
    await settle();
    account = ACCOUNT;
    app.fake.emit('onWsStatus', { connected: true });
    await settle();
    assert.equal(accountCard(app.account.state).title, 'Ada Lovelace');
  });

  it('names the remembered account from launch, before the server can be reached', async () => {
    const app = build({ getConfig: CONFIG, getAccount: ACCOUNT, getStatus: { connected: false } });
    await settle();
    assert.equal(accountCard(app.account.state).title, 'Ada Lovelace');
    assert.equal(accountCard(app.account.state).connection, 'reconnecting');
  });

  it('shows the latest answer when an earlier, slower one arrives after it', async () => {
    const answers: Array<(account: unknown) => void> = [];
    const app = build({ getConfig: CONFIG, getAccount: () => new Promise((resolve) => answers.push(resolve)) });
    await settle();
    app.fake.emit('onWsStatus', { connected: true });
    app.fake.emit('onWsStatus', { connected: false });
    app.fake.emit('onWsStatus', { connected: true });
    await settle();
    answers.at(-1)?.({ ...ACCOUNT, name: 'Grace Hopper' });
    for (const answer of answers.slice(0, -1)) answer(ACCOUNT);
    await settle();
    assert.equal(accountCard(app.account.state).title, 'Grace Hopper');
  });

  it('shows no account when asking for it fails', async () => {
    const app = await accountPage({ getAccount: () => Promise.reject(new Error('offline')) });
    assert.equal(app.account.state.account, null);
  });
});
