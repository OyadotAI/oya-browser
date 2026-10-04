/**
 * Unit tests for Sync on the account page: when the logins last reached the
 * server and how many sites it keeps, Sync now and its confirmation, and what
 * happens when the confirmation never comes or the send fails.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { canSync, syncLine } from '../../../../../../src/renderer/features/connection/view-models/sync-view-model.ts';
import { ago } from '../../../../../../src/renderer/ui/ago.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { build, settle } from '../harness.ts';

const CONFIG = { serverUrl: 'wss://oyabrowser.com/ws', apiKey: 'k' };
const MINUTE = 60_000;

/** The account page opened, over `answers`. */
async function accountPage(answers: Record<string, unknown> = {}) {
  const app = build({ getConfig: CONFIG, getAccount: null, ...answers });
  await settle();
  await app.dialog.open(true);
  return app;
}

/** The sync line as the page reads it. */
function text(app: Awaited<ReturnType<typeof accountPage>>): string {
  const line = syncLine(app.sync.state);
  return `${line.lead}${line.at ? ago(line.at) : ''}${line.count}`;
}

describe('sync', () => {
  it('says when the logins last reached the server, and how many sites it keeps', async () => {
    const lastSync = { at: Date.now() - 2 * MINUTE, sites: 937 };
    const app = await accountPage({ getConfig: { ...CONFIG, lastSync }, getStatus: { connected: true } });
    assert.equal(text(app), 'Logins synced 2 minutes ago · 937 sites');
    assert.equal(canSync(app.sync.state), true);
  });

  it('takes the later of the last Sync now and the last automatic send', async () => {
    const lastSync = { at: Date.now() - 60 * MINUTE, sites: 3 };
    const status = { connected: true, syncedAt: Date.now() - 5 * MINUTE };
    const app = await accountPage({ getConfig: { ...CONFIG, lastSync }, getStatus: status });
    assert.equal(text(app), 'Logins synced 5 minutes ago · 3 sites');
  });

  it('says "Last synced" while offline', async () => {
    const lastSync = { at: Date.now() - 2 * MINUTE, sites: 1 };
    const app = await accountPage({ getConfig: { ...CONFIG, lastSync } });
    assert.equal(text(app), 'Last synced 2 minutes ago · 1 site');
  });

  it('syncs now and says just now with the new count once the server confirms', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    await app.sync.save();
    assert.equal(app.fake.called('saveProfile').length, 1);
    assert.equal(canSync(app.sync.state), false);
    app.fake.emit('onProfileSaved', { sites: ['a.test'] });
    assert.equal(text(app), 'Logins synced just now · 1 site');
    assert.equal(canSync(app.sync.state), true);
    assert.equal(app.sync.state.note, '');
  });

  it('says what has to happen first when offline', async () => {
    const app = await accountPage();
    assert.equal(text(app), 'Logins sync when you reconnect');
    assert.equal(canSync(app.sync.state), false);
  });

  it('gives the button back, saying so, when the sync is never confirmed', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    try {
      const app = await accountPage({ getStatus: { connected: true } });
      await app.sync.save();
      mock.timers.tick(C.PROFILE_SAVE_TIMEOUT_MS);
      assert.equal(app.sync.state.note, 'Sync not confirmed. Try again.');
      assert.equal(canSync(app.sync.state), true);
    } finally {
      mock.timers.reset();
    }
  });

  it('says why the server refused a sync, keeping the last time', async () => {
    const app = await accountPage({ getStatus: { connected: true } });
    await app.sync.save();
    app.fake.emit('onProfileSaved', { error: 'Quota reached' });
    assert.equal(app.sync.state.note, 'Quota reached');
    assert.equal(app.sync.state.at, 0);
  });

  it('says why the send failed', async () => {
    const app = await accountPage({
      getStatus: { connected: true },
      saveProfile: () => Promise.reject(new Error('socket closed')),
    });
    await app.sync.save();
    assert.equal(app.sync.state.note, 'socket closed');
    assert.equal(app.sync.state.pending, false);
  });
});
