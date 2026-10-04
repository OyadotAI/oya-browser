/**
 * Unit tests for the shell dialog: opening on the commands or the account
 * page (read afresh), the overlay it asks for, closing, Advanced, the browser
 * id, and Copy.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { pageLabels } from '../../../../../../src/renderer/features/connection/view-models/shell-dialog-view-model.ts';
import { build, settle } from '../harness.ts';

describe('the shell dialog', () => {
  it('shows the server saved since start, after a pairing link retargets the browser', async () => {
    const config = { serverUrl: 'ws://localhost:3100/ws', browserName: 'Mine' };
    const app = build({ getConfig: () => config });
    await settle();
    config.serverUrl = 'wss://oyabrowser.com/ws';
    await app.dialog.open(true);
    assert.equal(app.dialog.state.server, 'wss://oyabrowser.com/ws');
  });

  it('keeps Advanced folded each time the account page opens', async () => {
    const app = build();
    app.dialog.setAdvanced(true);
    await app.dialog.open(true);
    assert.equal(app.dialog.state.advanced, false);
  });

  it('opens on the commands over the page, and gives the page back on close', async () => {
    const app = build();
    await app.dialog.open();
    assert.deepEqual([app.dialog.state.open, app.dialog.state.page], [true, 'commands']);
    assert.deepEqual(app.fake.called('showOverlay'), [['shell']]);
    app.dialog.close();
    assert.equal(app.dialog.state.open, false);
    assert.deepEqual(app.fake.called('hideOverlay'), [['shell']]);
  });

  it('does not give back a page it never took', () => {
    const app = build();
    app.dialog.close();
    assert.equal(app.fake.called('hideOverlay').length, 0);
  });

  it('names itself after the page in view', () => {
    assert.deepEqual(pageLabels('profile'), { title: 'Account', label: 'Account and connection' });
    assert.deepEqual(pageLabels('commands'), { title: 'Commands', label: 'Commands and settings' });
  });

  it('closes when the browser logs out', async () => {
    const app = build();
    await app.dialog.open(true);
    app.fake.emit('onModeChanged', 'setup');
    assert.equal(app.dialog.state.open, false);
  });

  it('follows the browser id as the connection reports it', async () => {
    const app = build({ getStatus: { connected: true, browserId: 'b-1' } });
    await settle();
    assert.equal(app.dialog.state.browserId, 'b-1');
    app.fake.emit('onWsStatus', { connected: false });
    assert.equal(app.dialog.state.browserId, '');
  });

  it('copies the browser id and says so', async () => {
    const copied: string[] = [];
    const app = build(
      { getStatus: { connected: true, browserId: 'b-1' } },
      { writeText: async (t) => void copied.push(t) },
    );
    await settle();
    await app.dialog.copy('browser ID');
    assert.deepEqual(copied, ['b-1']);
    assert.equal(app.dialog.state.copyStatus, 'Browser ID copied.');
  });

  it('says how to copy by hand when the clipboard refuses', async () => {
    const app = build({}, { writeText: () => Promise.reject(new Error('denied')) });
    await app.dialog.copy('server address');
    assert.equal(app.dialog.state.copyStatus, 'Could not copy. Select the server address to copy it manually.');
  });

  it('still opens the account page when the settings cannot be read', async () => {
    const app = build({ getConfig: () => Promise.reject(new Error('unreadable')) });
    await app.dialog.open(true);
    assert.deepEqual([app.dialog.state.open, app.dialog.state.page], [true, 'profile']);
  });
});
