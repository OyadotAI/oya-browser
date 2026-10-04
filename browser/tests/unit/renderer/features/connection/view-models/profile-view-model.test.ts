/**
 * Unit tests for "Browsing as" and "This device": the profile whose cookies
 * Oya uses, the device sites see, and renaming this browser.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  deviceText,
  personaText,
} from '../../../../../../src/renderer/features/connection/view-models/profile-view-model.ts';
import { build, settle } from '../harness.ts';

const DEVICE = { id: 'p-1', platform: 'Win32', timezone: 'Europe/London', locale: 'en-GB', screen: '1920x1080' };

describe('browsing as', () => {
  it('names the profile whose cookies Oya uses, in plain words', async () => {
    const app = build({ getStatus: { connected: true, profileName: 'Firefox · default' } });
    await settle();
    assert.deepEqual(personaText(app.profile.state.profileName), {
      name: '“Firefox · default”',
      detail: 'Oya signs in to sites with this profile’s cookies and logins.',
    });
  });

  it("calls the default profile Oya's own", () => {
    assert.equal(personaText('Default').name, 'Oya’s own profile');
  });
});

describe('this device', () => {
  it('shows the device this browser runs as, in words', async () => {
    const app = build({ getFingerprint: DEVICE });
    await settle();
    assert.equal(deviceText(app.profile.state.device), 'Windows · Europe/London · en-GB · 1920x1080');
  });

  it('follows a change of persona', async () => {
    const app = build({ getFingerprint: DEVICE });
    await settle();
    app.fake.emit('onFingerprintChanged', { ...DEVICE, platform: 'MacIntel', timezone: 'America/New_York' });
    assert.match(deviceText(app.profile.state.device), /^Mac · America\/New_York/);
  });

  it('says so when no persona has been assigned yet, or it cannot be read', async () => {
    const none = build({ getFingerprint: null });
    const failed = build({ getFingerprint: () => Promise.reject(new Error('x')) });
    await settle();
    assert.equal(deviceText(none.profile.state.device), 'This computer, until you connect');
    assert.equal(deviceText(failed.profile.state.device), 'This computer, until you connect');
  });

  it('saves a new name as it is changed, and nothing for an empty or unchanged one', async () => {
    const app = build({ getConfig: { browserName: 'Desk' }, saveConfig: true });
    await settle();
    assert.equal(app.profile.state.name, 'Desk');
    app.profile.type('  ');
    await app.profile.rename();
    assert.equal(app.profile.state.name, 'Desk', 'an empty name is put back');
    await app.profile.rename();
    app.profile.type('Studio Mac');
    await app.profile.rename();
    assert.deepEqual(app.fake.called('saveConfig'), [[{ browserName: 'Studio Mac' }]]);
    assert.equal(app.profile.state.nameStatus, 'Name saved.');
  });

  it('says why a name could not be saved, and clears that once typing again', async () => {
    const app = build({ saveConfig: () => Promise.reject(new Error('read-only')) });
    await settle();
    app.profile.type('New');
    await app.profile.rename();
    assert.equal(app.profile.state.nameStatus, 'read-only');
    app.profile.type('Newer');
    assert.equal(app.profile.state.nameStatus, '');
  });
});
