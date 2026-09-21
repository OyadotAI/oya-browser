/**
 * Unit tests for the profile section of the connection dialog
 * (renderer/connection/profile.js): which device this browser runs as.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

const DEVICE = { id: 'p-1', platform: 'Win32', timezone: 'Europe/London', locale: 'en-GB', screen: '1920x1080' };

describe('the profile section', () => {
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
});
