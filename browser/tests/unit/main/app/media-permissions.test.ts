/**
 * Unit tests for src/main/app/media-permissions.ts: device permissions are
 * explicit, remembered per profile and site, and safe across asynchronous
 * prompts; a remembered grant never bypasses the gate.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fixture, details } from '../../support/media-fixture.ts';

it('queued device consent cannot capture a document or profile that replaced its original requester', async () => {
  for (const revoke of [
    (f) => f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false }),
    (f) => (f.deps.persona.partitionName = () => 'persist:two'),
    (f) => (f.state.interactive = false),
    (f) => (f.state.destroyed = true),
  ]) {
    const f = fixture();
    const pending = Promise.withResolvers<boolean>();
    const first = f.service.gate.locked(
      () => true,
      () => pending.promise,
    );
    const queued = f.service.request(f.contents as any, details as any);
    revoke(f);
    pending.resolve(true);
    await first;
    assert.equal(await queued, false);
    assert.deepEqual(f.prompts, []);
    assert.deepEqual(f.devices, []);
  }
});
it('requires explicit site consent and macOS consent before granting the requested device', async () => {
  const f = fixture();
  assert.equal(f.check(), false);
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  assert.deepEqual(f.devices, ['microphone']);
  assert.equal(f.check(), true);
  assert.equal(f.check('video'), false);
  assert.equal(f.check('unknown'), false);
  assert.equal(f.prompts[0].defaultId, 0);
  assert.match(f.prompts[0].detail, /remembered for this site/);
});
it('cancellation never requests OS permission', async () => {
  const f = fixture();
  f.state.response = 0;
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  assert.deepEqual(f.devices, []);
});
it('Block is not remembered: the next request asks again', async () => {
  const f = fixture();
  f.state.response = 0;
  await f.service.request(f.contents as any, details as any);
  f.state.response = 1;
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  assert.equal(f.prompts.length, 2);
});
it('OS refusal keeps site consent without repeating Allow or reporting device access as granted', async () => {
  const f = fixture();
  f.state.os = false;
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  assert.equal(f.check(), false);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'audio'), true);
  assert.match(f.prompts[1].detail, /System Settings/);
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  f.state.os = true;
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  assert.equal(f.check(), true);
  assert.equal(f.prompts.filter((p) => p.message === 'Allow microphone?').length, 1);
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
it('a remembered grant is still refused for an agent-driven tab, a governed browser and a background tab', async () => {
  for (const block of [
    (f) => (f.state.interactive = false),
    (f) => (f.deps.governance.configuration = {}),
    (f) => (f.deps.tabs.getActiveView = () => null),
  ]) {
    const f = fixture();
    f.deps.sitePermissions.grant('persist:one', 'https://call.test', ['audio']);
    block(f);
    assert.equal(f.check(), false);
    assert.equal(await f.service.request(f.contents as any, details as any), false);
    assert.deepEqual(f.devices, [], 'never reaches the OS');
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
/** A new top-level document, as Electron describes a reload. */
const reload = { isMainFrame: true, isSameDocument: false };
it('keeps a grant across pushState and iframe loads, which leave the document in place', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.state.url = 'https://call.test/room?joined=1';
  f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: true });
  f.contents.emit('did-start-navigation', { isMainFrame: false, isSameDocument: false });
  assert.equal(f.check(), true);
});
it('works in a tab the person owns while agents drive other tabs', async () => {
  const f = fixture();
  f.deps.agentTab.humanMay = (id) => id === 1;
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  f.deps.agentTab.humanMay = () => false;
  assert.equal(f.check(), false);
});
it('a remembered Allow survives a reload and the next meeting without asking again, but still asks macOS', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.contents.emit('did-start-navigation', reload);
  assert.equal(f.check(), true);
  f.state.url = 'https://call.test/other-room';
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  assert.equal(f.prompts.length, 1, 'asked once');
  assert.deepEqual(f.devices, ['microphone', 'microphone']);
});
it('a remembered grant is not consulted for another profile or another site', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.deps.persona.partitionName = () => 'persist:two';
  assert.equal(f.check(), false);
  f.deps.persona.partitionName = () => 'persist:one';
  f.state.url = 'https://other.test/';
  assert.equal(f.check(), false);
});
it('an approval racing navigation is neither granted nor remembered', async () => {
  const f = fixture();
  const pending = Promise.withResolvers();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const request = f.service.request(f.contents as any, details as any);
  f.contents.emit('did-start-navigation', reload);
  pending.resolve({ response: 1 });
  assert.equal(await request, false);
  assert.equal(f.check(), false);
  assert.equal(f.contents.listenerCount('did-start-navigation'), 1);
});
it('explains a macOS denial once per page, not once per request', async () => {
  const f = fixture();
  f.state.os = false;
  await f.service.request(f.contents as any, details as any);
  await f.service.request(f.contents as any, details as any);
  assert.equal(f.prompts.filter((p) => /System Settings/.test(p.detail)).length, 1);
  f.contents.emit('did-start-navigation', reload);
  await f.service.request(f.contents as any, details as any);
  assert.equal(f.prompts.filter((p) => /System Settings/.test(p.detail)).length, 2);
});
it('rechecks control after OS consent and safely handles OS errors', async () => {
  const f = fixture();
  f.deps.electron.systemPreferences.askForMediaAccess = async () => {
    f.state.interactive = false;
    return true;
  };
  assert.equal(await f.service.request(f.contents as any, details as any), false);
  f.state.interactive = true;
  assert.equal(f.check(), true, 'site consent remains after the person takes the tab back');
  f.deps.electron.systemPreferences.askForMediaAccess = async () => {
    throw new Error('OS failure');
  };
  assert.equal(await f.service.request(f.contents as any, details as any), false);
});
it('a corrupt permissions file is treated as empty', async () => {
  const f = fixture('darwin', { read: () => 'garbage', write: () => {} } as any);
  assert.equal(f.check(), false);
  assert.equal(await f.service.request(f.contents as any, details as any), true);
});
it('ships the macOS usage descriptions and capture entitlements', () => {
  const pkg = JSON.parse(readFileSync(new URL('../../../../package.json', import.meta.url), 'utf8'));
  const plist = readFileSync(new URL('../../../../build/entitlements.mac.plist', import.meta.url), 'utf8');
  assert.ok(pkg.build.mac.extendInfo.NSMicrophoneUsageDescription);
  assert.ok(pkg.build.mac.extendInfo.NSCameraUsageDescription);
  assert.ok(pkg.build.mac.extendInfo.NSScreenCaptureUsageDescription);
  assert.match(plist, /com.apple.security.device.audio-input/);
  assert.match(plist, /com.apple.security.device.camera/);
});
