/**
 * Unit tests for src/main/app/screen-share.ts: sharing goes through the same
 * gate as the camera, is remembered per profile, hands over an explicitly chosen source
 * as video only, and never uses the system picker (which would skip the gate).
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { ScreenShare } from '../../../../src/main/app/screen-share.ts';
import { fixture as mediaFixture } from '../../support/media-fixture.ts';

/** A screen share over the camera fixture: its gate, profile and store, plus a capturer and the macOS switch. */
function fixture(platform = 'darwin') {
  const f = mediaFixture(platform);
  const frame = { id: 'main' };
  (f.contents as any).mainFrame = frame;
  const electron = f.deps.electron as any;
  electron.webContents = { fromFrame: (asked) => (asked === frame ? f.contents : null) };
  const picks = [];
  const opened = [];
  electron.shell = {
    openExternal: async (url) => {
      opened.push(url);
    },
  };
  let probes = 0;
  const picker = {
    requestOSPermission: async () => {
      probes++;
    },
    pick: async (...args) => (picks.push(args), { video: { id: 'screen:0', name: 'Screen 1' } }),
  };
  electron.systemPreferences.getMediaAccessStatus = () => 'granted';
  const started = [];
  const shareIndicator = { started: (...args) => started.push(args) };
  const deps = { ...f.deps, mediaPermissions: f.service, screenPicker: picker, shareIndicator };
  const share = new ScreenShare(deps as any, platform);
  const ses: any = {};
  let handler;
  ses.setDisplayMediaRequestHandler = (fn, opts) => ((handler = fn), (ses.opts = opts));
  share.install(ses);
  const request = (overrides = {}) =>
    new Promise((resolve) => handler({ frame, securityOrigin: 'https://call.test', ...overrides }, resolve));
  return { ...f, frame, electron, ses, request, share, picker, picks, opened, probes: () => probes, started };
}

it('requires top-frame screen consent on the native media preflight', async () => {
  const f = fixture();
  const details = { isMainFrame: true, securityOrigin: 'https://call.test', mediaTypes: [] };
  const ask = (overrides = {}) => f.share.request(f.contents as any, { ...details, ...overrides } as any);
  assert.equal(await ask({ isMainFrame: false }), false);
  assert.equal(await ask({ securityOrigin: 'https://other.test' }), false);
  assert.deepEqual(f.prompts, []);
  f.state.response = 0;
  assert.equal(await ask(), false);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'screen'), false);
  f.state.response = 1;
  assert.equal(await ask(), true);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'screen'), true);
  f.state.interactive = false;
  assert.equal(await ask(), false);
});

it('installs without the system picker, so every request reaches the gate', () => {
  assert.equal(fixture().ses.opts, undefined);
});
it('remembers site consent but asks for a source on every share, as video only', async () => {
  const f = fixture();
  assert.deepEqual(await f.request({ audioRequested: true }), { video: { id: 'screen:0', name: 'Screen 1' } });
  assert.match(f.prompts[0].message, /Share your screen with https:\/\/call.test/);
  assert.equal(f.prompts[0].defaultId, 0);
  assert.deepEqual(await f.request(), { video: { id: 'screen:0', name: 'Screen 1' } });
  assert.equal(f.prompts.length, 1, 'remembered');
  assert.equal(f.picks.length, 2, 'the source is never remembered');
});
it('refuses agent, governed, background, http and iframe requesters before reading the store', async () => {
  for (const block of [
    (f) => (f.state.interactive = false),
    (f) => (f.deps.governance.configuration = {}),
    (f) => (f.deps.tabs.getActiveView = () => null),
    (f) => (f.state.url = 'http://call.test/'),
    (f) => (f.frame.id = 'child'),
  ]) {
    const f = fixture();
    f.deps.sitePermissions.grant('persist:one', 'https://call.test', ['screen']);
    block(f);
    const overrides = f.frame.id === 'child' ? { frame: { id: 'child' } } : {};
    assert.equal(await f.request(overrides), null);
    assert.deepEqual(f.prompts, []);
  }
});
it('Block answers null and remembers nothing', async () => {
  const f = fixture();
  f.state.response = 0;
  assert.equal(await f.request(), null);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'screen'), false);
});
it('macOS denial requests enumeration before explaining the specific setting, and refuses capture', async () => {
  const f = fixture();
  f.electron.systemPreferences.getMediaAccessStatus = () => 'denied';
  assert.equal(await f.request(), null);
  assert.equal(await f.request(), null);
  assert.equal(f.probes(), 2);
  assert.equal(f.picks.length, 0);
  assert.equal(f.prompts.filter((p) => /Screen & System Audio Recording/.test(p.detail)).length, 1);
  assert.deepEqual(f.opened, ['x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture']);
});

it('first-time native enumeration may grant access and proceed to an explicit source choice', async () => {
  const f = fixture();
  f.electron.systemPreferences.getMediaAccessStatus = () => 'denied';
  f.picker.requestOSPermission = async () => {
    f.electron.systemPreferences.getMediaAccessStatus = () => 'granted';
  };
  assert.ok(await f.request());
  assert.equal(f.picks.length, 1);
  assert.deepEqual(f.opened, []);
});

it('navigation while macOS asks for access cannot grant the replacement document', async () => {
  const f = fixture();
  f.electron.systemPreferences.getMediaAccessStatus = () => 'denied';
  f.picker.requestOSPermission = async () => {
    f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
  };
  assert.equal(await f.request(), null);
  assert.deepEqual(f.picks, []);
  assert.deepEqual(f.opened, []);
});
it('a page that navigated during the dialog gets null', async () => {
  const f = fixture();
  const pending = Promise.withResolvers();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const answer = f.request();
  f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false });
  pending.resolve({ response: 1 });
  assert.equal(await answer, null);
});
it('a picker failure answers null rather than throwing into Electron', async () => {
  const f = fixture();
  f.picker.pick = async () => {
    throw new Error('no screens');
  };
  assert.equal(await f.request(), null);
});

it('cancelling the picker never falls back to an automatic screen', async () => {
  const f = fixture();
  f.picker.pick = async () => null;
  assert.equal(await f.request(), null);
  assert.equal(f.service.gate.inCall(f.contents as any), false, 'a cancelled picker is not a call');
});

it('a chosen screen source marks the tab in a call', async () => {
  const f = fixture();
  await f.request();
  assert.equal(f.service.gate.inCall(f.contents as any), true);
});

it('refuses a screen source if control, document or profile changes during capture selection', async () => {
  for (const revoke of [
    (f) => (f.state.interactive = false),
    (f) => f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false }),
    (f) => (f.deps.persona.partitionName = () => 'persist:two'),
  ]) {
    const f = fixture();
    f.picker.pick = async () => {
      revoke(f);
      return { video: { id: 'screen:0' } };
    };
    assert.equal(await f.request(), null);
  }
});

it('a prior microphone denial does not suppress Screen Recording recovery', async () => {
  const f = fixture();
  const generation = f.service.gate.watch(f.contents as any);
  await f.service.gate.osDenied(f.contents as any, generation);
  f.electron.systemPreferences.getMediaAccessStatus = () => 'denied';
  assert.equal(await f.request(), null);
  assert.equal(f.opened.length, 1);
});

it('an approved share is reported to the indicator; a cancelled or revoked one is not', async () => {
  const f = fixture();
  await f.request();
  assert.deepEqual(f.started, [[{ id: 'screen:0', name: 'Screen 1' }, f.contents]]);
  f.picker.pick = async () => null;
  await f.request();
  f.picker.pick = async () => ((f.state.interactive = false), { video: { id: 'screen:0' } });
  await f.request();
  assert.equal(f.started.length, 1);
});
