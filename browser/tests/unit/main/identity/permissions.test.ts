/**
 * Unit tests for src/main/identity/permissions.ts: a page gets unasked only what Chrome
 * gives unasked, on requests and on checks alike.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { installPermissions } from '../../../../src/main/identity/permissions.ts';

/** A session that keeps the handlers it is given. */
function fakeSession() {
  const ses: any = {};
  ses.setPermissionRequestHandler = (fn: any) => (ses.request = fn);
  ses.setPermissionCheckHandler = (fn: any) => (ses.check = fn);
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
      ses.request(null, permission, (answer: boolean) => (granted = answer));
      assert.equal(granted, false, `${permission} is not granted on request`);
      assert.equal(ses.check(null, permission), false, `${permission} does not read as granted`);
    }
  });

  it('allows what Chrome allows without asking', () => {
    const ses = fakeSession();
    installPermissions(ses);
    for (const permission of ['fullscreen', 'pointerLock', 'clipboard-sanitized-write', 'sensors']) {
      let granted = null;
      ses.request(null, permission, (answer: boolean) => (granted = answer));
      assert.equal(granted, true);
      assert.equal(ses.check(null, permission), true);
    }
  });
});

it('denies automatic external launches and delegates only the explicit confirmation path', () => {
  const ses = fakeSession(),
    calls = [],
    contents = {};
  installPermissions(ses, (url, source) => calls.push({ url, source }));
  const url = 'zoommtg://zoom.us/join?confno=123';
  let granted;
  ses.request(contents, 'openExternal', (answer) => (granted = answer), { externalURL: url });
  assert.equal(granted, false);
  assert.equal(ses.check(contents, 'openExternal'), false);
  assert.deepEqual(calls, [{ url, source: contents }]);
  ses.request(contents, 'openExternal', () => {}, {});
  ses.request(contents, 'media', () => {}, { externalURL: url });
  assert.equal(calls.length, 1);
});

it('routes media requests and checks through the explicit consent service', async () => {
  const ses = fakeSession(),
    contents = {},
    details = { mediaTypes: ['audio'] };
  const calls = [];
  const media = {
    request: async (...args) => {
      calls.push(args);
      return true;
    },
    check: () => true,
  };
  installPermissions(ses, undefined, media as any);
  const granted = await new Promise((resolve) => ses.request(contents, 'media', resolve, details));
  assert.equal(granted, true);
  assert.deepEqual(calls, [[contents, details]]);
  assert.equal(ses.check(contents, 'media', 'https://call.test', {}), true);
  media.request = async () => {
    throw new Error('permission unavailable');
  };
  assert.equal(await new Promise((resolve) => ses.request(contents, 'media', resolve, details)), false);
});
