/**
 * Unit tests for main/permissions.cjs: a page gets unasked only what Chrome
 * gives unasked, on requests and on checks alike.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { installPermissions } = require('../../../main/permissions.cjs');

/** A session that keeps the handlers it is given. */
function fakeSession() {
  const ses = {};
  ses.setPermissionRequestHandler = (fn) => (ses.request = fn);
  ses.setPermissionCheckHandler = (fn) => (ses.check = fn);
  return ses;
}

describe('installPermissions', () => {
  it('refuses the camera, microphone, location, notifications and MIDI, which Chrome would ask about', () => {
    const ses = fakeSession();
    installPermissions(ses);
    for (const permission of [
      'media',
      'geolocation',
      'notifications',
      'midi',
      'midiSysex',
      'clipboard-read',
      'openExternal',
    ]) {
      let granted = null;
      ses.request(null, permission, (answer) => (granted = answer));
      assert.equal(granted, false, `${permission} is not granted on request`);
      assert.equal(ses.check(null, permission), false, `${permission} does not read as granted`);
    }
  });

  it('allows what Chrome allows without asking', () => {
    const ses = fakeSession();
    installPermissions(ses);
    for (const permission of ['fullscreen', 'pointerLock', 'clipboard-sanitized-write', 'sensors']) {
      let granted = null;
      ses.request(null, permission, (answer) => (granted = answer));
      assert.equal(granted, true);
      assert.equal(ses.check(null, permission), true);
    }
  });
});
