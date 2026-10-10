/**
 * Unit tests for the connection pill: connected with the browser id,
 * connecting while a saved key waits for its first status, offline, the
 * status at start switching to browsing, and opening the account page.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { pillFor } from '../../../../../../src/renderer/features/connection/view-models/connection-pill-view-model.ts';
import { build, settle } from '../harness.ts';

describe('the connection pill', () => {
  it('shows connection failure with the safe recovery instruction', () => {
    const status = pillFor({ connected: false, failure: 'Restart Oya to retry.' });
    assert.equal(status.label, 'Connection failed');
    assert.equal(status.title, 'Restart Oya to retry.');
  });
  it('shows connected, with the browser id on hover', () => {
    assert.deepEqual(pillFor({ connected: true, browserId: 'b-1' }), {
      className: 'conn-pill ok',
      label: 'Connected',
      title: 'Connected, b-1',
    });
  });

  it('shows offline, inviting a click', () => {
    assert.deepEqual(pillFor({ connected: false }), {
      className: 'conn-pill',
      label: 'Offline',
      title: 'Not connected, click to configure',
    });
  });

  it('says "Connecting…" while a saved key waits for its first status', async () => {
    let answer: (s: unknown) => void = () => {};
    const status = new Promise((resolve) => (answer = resolve));
    const app = build({ getConfig: { apiKey: 'k' }, getStatus: () => status });
    await settle();
    assert.deepEqual([app.pill.state.className, app.pill.state.label], ['conn-pill trying', 'Connecting…']);
    answer({ connected: true, browsing: true });
    await settle();
    assert.equal(app.pill.state.label, 'Connected');
  });

  it('switches the shell to browsing and connected from the status at start', async () => {
    const app = build({ getStatus: { connected: true, browsing: true } });
    await settle();
    assert.deepEqual([app.shell.state.mode, app.shell.state.connected], ['browsing', true]);
  });

  it('follows the connection', async () => {
    const app = build();
    await settle();
    app.fake.emit('onWsStatus', { connected: true });
    assert.equal(app.pill.state.label, 'Connected');
  });

  it('opens the account page when clicked', async () => {
    const app = build();
    app.pill.click();
    await settle();
    assert.deepEqual([app.dialog.state.open, app.dialog.state.page], [true, 'profile']);
  });
});
