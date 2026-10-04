/**
 * Unit tests for the update pill: what it says in each updater state, when
 * it shows and is highlighted, checking on click, restarting into a staged
 * update, and a failed check.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  updateLabel,
  updatesFor,
} from '../../../../../../src/renderer/features/connection/view-models/updates-view-model.ts';
import { build, settle } from '../harness.ts';

describe('the update pill', () => {
  it('says what the updater is doing', () => {
    assert.equal(updateLabel({ state: 'checking' }), 'Checking…');
    assert.equal(updateLabel({ state: 'available', version: '2.0' }), 'Update 2.0 available');
    assert.equal(updateLabel({ state: 'downloading', version: '2.0', percent: 40 }), 'Downloading 2.0… 40%');
    assert.equal(updateLabel({ state: 'ready', version: '2.0' }), 'Update 2.0 ready, Restart');
    assert.equal(updateLabel({ state: 'error', current: '1.0' }), 'v1.0, check failed');
    assert.equal(updateLabel({ state: 'unsupported', current: '1.0' }), 'v1.0');
    assert.equal(updateLabel({ state: 'none', current: '1.0' }), 'v1.0, up to date');
  });

  it('shows itself only while there is something to say, highlighted while an update is coming', () => {
    assert.deepEqual([updatesFor({ state: 'none' }).hidden, updatesFor({ state: 'error' }).hidden], [true, false]);
    assert.equal(updatesFor({ state: 'downloading' }).attention, true);
    assert.equal(updatesFor({ state: 'downloading' }).disabled, true);
    assert.equal(updatesFor({ state: 'ready' }).title, 'Restart to finish updating');
    assert.equal(updatesFor({ state: 'unsupported' }).title, 'Updates apply to installed builds');
  });

  it('shows the running version in the footer', async () => {
    const app = build({ getUpdateStatus: { state: 'none', current: '1.2.3' } });
    await settle();
    assert.equal(app.updates.state.version, 'Oya Browser · v1.2.3');
  });

  it('checks for an update on click', async () => {
    const app = build({ checkForUpdates: { state: 'available', version: '2.0', current: '1.0' } });
    await app.updates.click();
    assert.equal(app.updates.state.text, 'Update 2.0 available');
    assert.equal(app.fake.called('installUpdate').length, 0);
  });

  it('restarts into a staged update on click', async () => {
    const app = build();
    await settle();
    app.fake.emit('onUpdateStatus', { state: 'ready', version: '2.0' });
    await app.updates.click();
    assert.equal(app.fake.called('installUpdate').length, 1);
    assert.equal(app.updates.state.text, 'Restarting…');
  });

  it('says the check failed when it cannot be made', async () => {
    const app = build({
      getUpdateStatus: { state: 'none', current: '1.0' },
      checkForUpdates: () => Promise.reject(new Error('offline')),
    });
    await settle();
    await app.updates.check();
    assert.equal(app.updates.state.text, 'v1.0, check failed');
  });
});
