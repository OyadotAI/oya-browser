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

it('routes notification requests and checks through the notification consent service', async () => {
  const ses = fakeSession(),
    contents = {},
    calls = [];
  const notifications = {
    request: async (...args) => (calls.push(args), true),
    check: (...args) => (calls.push(args), true),
  };
  installPermissions(ses, undefined, undefined, undefined, notifications as any);
  const granted = await new Promise((resolve) =>
    ses.request(contents, 'notifications', resolve, { requestingUrl: 'https://mail.test/' }),
  );
  assert.equal(granted, true);
  assert.equal(ses.check(contents, 'notifications', 'https://mail.test/'), true);
  assert.deepEqual(calls, [
    [contents, 'https://mail.test/'],
    [contents, 'https://mail.test/'],
  ]);
  notifications.request = async () => {
    throw new Error('dialog failed');
  };
  assert.equal(await new Promise((resolve) => ses.request(contents, 'notifications', resolve, {})), false);
});

it('permits display capture only after installing the display-media consent handler', async () => {
  const ses = fakeSession();
  const installed = [];
  const screen = { install: (session) => installed.push(session), request: async () => true };
  const request = () => new Promise((resolve) => ses.request({}, 'display-capture', resolve, {}));
  installPermissions(ses);
  assert.equal(await request(), false);
  installPermissions(ses, undefined, undefined, screen as any);
  assert.equal(await request(), false, 'a session without the display-media hook cannot share');
  assert.deepEqual(installed, []);
  ses.setDisplayMediaRequestHandler = () => {};
  installPermissions(ses, undefined, undefined, screen as any);
  assert.deepEqual(installed, [ses]);
  assert.equal(await request(), true);
});

it('routes native empty-device preflights to screen consent and keeps camera requests separate', async () => {
  const ses = fakeSession();
  const calls = [];
  const contents = {};
  const media = { request: async (_contents, details) => (calls.push(['device', details]), false) };
  const screen = { install: () => {}, request: async (_contents, details) => (calls.push(['screen', details]), true) };
  const ask = (details) => new Promise((resolve) => ses.request(contents, 'media', resolve, details));
  const display = { mediaTypes: [], isMainFrame: true, securityOrigin: 'https://call.test/' };
  installPermissions(ses, undefined, media as any, screen as any);
  assert.equal(await ask(display), false, 'without a display handler the preflight is refused');
  ses.setDisplayMediaRequestHandler = () => {};
  installPermissions(ses, undefined, media as any, screen as any);
  assert.equal(await ask(display), true);
  assert.deepEqual(calls.pop(), ['screen', display]);
  for (const details of [{ mediaTypes: ['audio'] }, { mediaTypes: ['video'] }, {}]) {
    assert.equal(await ask(details), false);
    assert.deepEqual(calls.pop(), ['device', details]);
  }
  screen.request = async () => false;
  assert.equal(await ask(display), false, 'a refused screen decision is preserved');
});
