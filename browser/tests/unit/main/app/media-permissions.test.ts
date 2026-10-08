/** Native device permissions are explicit, document-scoped and safe across asynchronous prompts. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { readFileSync } from 'node:fs';
import { MediaPermissions } from '../../../../src/main/app/media-permissions.ts';
/** Requests describe the actual secure requesting frame. */
const details = {
  mediaTypes: ['audio'],
  requestingUrl: 'https://call.test/room',
  securityOrigin: 'https://call.test',
  isMainFrame: true,
};
/** Fake native devices and OS prompts; no real hardware is touched. */
function fixture(platform = 'darwin') {
  const state = { url: details.requestingUrl, interactive: true, response: 1, os: true, destroyed: false };
  const contents = Object.assign(new EventEmitter(), { getURL: () => state.url, isDestroyed: () => state.destroyed });
  const prompts = [],
    devices = [];
  const deps = {
    electron: {
      dialog: {
        showMessageBox: async (options) => {
          prompts.push(options);
          return { response: state.response };
        },
      },
      systemPreferences: {
        askForMediaAccess: async (type) => {
          devices.push(type);
          return state.os;
        },
      },
    },
    shell: { window: null },
    tabs: { getActiveView: () => ({ webContents: contents }) },
    control: { snapshot: () => ({ interactive: state.interactive }) },
    governance: { configuration: null },
  };
  const service = new MediaPermissions(deps as any, platform);
  const check = (mediaType = 'audio') =>
    service.check(contents as any, 'https://call.test', { mediaType, isMainFrame: true } as any);
  return { state, contents, deps, prompts, devices, service, check };
}
it('requires explicit site consent and macOS consent before granting the requested device', async () => {
  const f = fixture();
  assert.equal(f.check(), false);
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  assert.deepEqual(f.devices, ['microphone']);
  assert.equal(f.check(), true);
  assert.equal(f.check('video'), false);
  assert.equal(f.check('unknown'), false);
  assert.equal(f.prompts[0].defaultId, 0);
});
it('cancellation never requests OS permission', async () => {
  const f = fixture();
  f.state.response = 0;
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  assert.deepEqual(f.devices, []);
});
it('OS refusal is explained and never reported as granted', async () => {
  const f = fixture();
  f.state.os = false;
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  assert.equal(f.check(), false);
  assert.match(f.prompts[1].detail, /System Settings/);
});
it('microphone and camera consent accumulate separately on non-macOS platforms', async () => {
  const f = fixture('win32');
  await f.service.request(f.contents as any, details as any);
  await f.service.request(f.contents as any, { ...details, mediaTypes: ['video'] } as any);
  assert.equal(f.check(), true);
  assert.equal(f.check('video'), true);
  assert.deepEqual(f.devices, []);
});
it('refuses agent, managed, background, destroyed and insecure requesters', async () => {
  for (const block of [
    (f) => (f.state.interactive = false),
    (f) => (f.deps.governance.configuration = {}),
    (f) => (f.deps.tabs.getActiveView = () => null),
    (f) => (f.state.destroyed = true),
    (f) => (f.state.url = 'http://call.test/'),
  ]) {
    const f = fixture();
    block(f);
    assert.equal(await f.service.request(f.contents as any, details as any), false);
    assert.deepEqual(f.prompts, []);
  }
});
it('rejects cross-origin frames and unknown device requests', async () => {
  const f = fixture();
  for (const change of [
    { securityOrigin: 'https://evil.test' },
    { mediaTypes: [] },
    { mediaTypes: ['display'] },
    { mediaTypes: undefined },
  ])
    assert.equal(await f.service.request(f.contents as any, { ...details, ...change } as any), false);
  assert.deepEqual(f.prompts, []);
});
it('revokes checks on same-URL reload and rejects an approval racing navigation', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.contents.emit('did-start-navigation');
  assert.equal(f.check(), false);
  const pending = Promise.withResolvers();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const request = f.service.request(f.contents as any, details as any);
  f.contents.emit('did-start-navigation');
  pending.resolve({ response: 1 });
  assert.equal(await request, false);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 1);
});
it('rechecks control after OS consent and safely handles OS errors', async () => {
  const f = fixture();
  f.deps.electron.systemPreferences.askForMediaAccess = async () => {
    f.state.interactive = false;
    return true;
  };
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  f.state.interactive = true;
  f.deps.electron.systemPreferences.askForMediaAccess = async () => {
    throw new Error('OS failure');
  };
  assert.equal(await f.service.request(f.contents as any, details as any), false);
});
it('ships both macOS usage descriptions and capture entitlements', () => {
  const pkg = JSON.parse(readFileSync(new URL('../../../../package.json', import.meta.url), 'utf8'));
  const plist = readFileSync(new URL('../../../../build/entitlements.mac.plist', import.meta.url), 'utf8');
  assert.ok(pkg.build.mac.extendInfo.NSMicrophoneUsageDescription);
  assert.ok(pkg.build.mac.extendInfo.NSCameraUsageDescription);
  assert.match(plist, /com.apple.security.device.audio-input/);
  assert.match(plist, /com.apple.security.device.camera/);
});
