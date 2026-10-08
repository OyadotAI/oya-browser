/** Zoom links leave the sandbox only after a person consents; no OS apps are launched by tests. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { ExternalApps, zoomMeetingLink } from '../../../../src/main/app/external-apps.ts';
import { TabWindows } from '../../../../src/main/tabs/tab-windows.ts';
import { NavigationHandlers } from '../../../../src/main/ipc/navigation.ts';
import { mainCtx } from '../../support/main-ctx.cjs';
const link = 'zoommtg://us02web.zoom.us/join?confno=123456789&pwd=example%2Bsecret';
/** Drain native-prompt continuations without timers. */
const settle = () => new Promise((resolve) => setImmediate(resolve));
/** A controllable active page and fake native dialog/launcher. */
function fixture() {
  const state = { interactive: true, destroyed: false, url: 'https://us02web.zoom.us/j/123?pwd=private', response: 1 };
  const contents = Object.assign(new EventEmitter(), { getURL: () => state.url, isDestroyed: () => state.destroyed });
  const opened = [],
    asked = [];
  const deps = {
    electron: {
      shell: {
        openExternal: async (url) => {
          opened.push(url);
        },
      },
      dialog: {
        showMessageBox: async (options) => {
          asked.push(options);
          return { response: state.response };
        },
      },
    },
    shell: { window: null },
    tabs: { getActiveView: () => ({ webContents: contents }) },
    control: { snapshot: () => ({ interactive: state.interactive }) },
    governance: { configuration: null },
  };
  return { state, contents, opened, asked, deps, service: new ExternalApps(deps as any) };
}
it('accepts Zoom meeting joins without losing query values', () => {
  assert.equal(zoomMeetingLink(link), link);
  assert.equal(zoomMeetingLink('zoommtg://zoom.us/join?confno=123'), 'zoommtg://zoom.us/join?confno=123');
});
it('rejects arbitrary schemes, commands, spoofed hosts, credentials and malformed links', () => {
  for (const raw of [
    'file:///private',
    'ms-settings:appsfeatures',
    'zoommtg://zoom.us.evil.test/join',
    'zoommtg://evilzoom.us/join',
    'zoommtg://user:pass@zoom.us/join',
    'zoommtg://zoom.us:80/join',
    'zoommtg://zoom.us/start',
    'zoommtg://zoom.us/join#x',
    'zoommtg://zoom.us/jo\nin',
    'zoommtg://zoom.us/join?x=' + 'x'.repeat(8192),
  ])
    assert.equal(zoomMeetingLink(raw), null, raw.slice(0, 60));
});
it('asks before opening and never exposes meeting secrets in the confirmation', async () => {
  const f = fixture();
  assert.equal(f.service.request(link, f.contents as any), true);
  assert.equal(f.opened.length, 0);
  await settle();
  assert.deepEqual(f.opened, [link]);
  assert.equal(f.asked[0].defaultId, 0);
  assert.equal(f.asked[0].cancelId, 0);
  assert.match(f.asked[0].detail, /https:\/\/us02web.zoom.us/);
  assert.doesNotMatch(JSON.stringify(f.asked), /private|confno|example/);
});
it('cancel never launches an app', async () => {
  const f = fixture();
  f.state.response = 0;
  f.service.request(link, f.contents as any);
  await settle();
  assert.deepEqual(f.opened, []);
});
it('agent, managed, background and destroyed surfaces cannot launch or prompt', async () => {
  for (const block of [
    (f) => (f.state.interactive = false),
    (f) => (f.deps.governance.configuration = {}),
    (f) => (f.deps.tabs.getActiveView = () => null),
    (f) => (f.state.destroyed = true),
  ]) {
    const f = fixture();
    block(f);
    assert.equal(f.service.request(link, f.contents as any), true);
    await settle();
    assert.deepEqual(f.asked, []);
    assert.deepEqual(f.opened, []);
  }
});
it('rechecks control, active surface and source after confirmation', async () => {
  for (const change of [
    (f) => (f.state.interactive = false),
    (f) => (f.state.url = 'https://other.test/'),
    (f) => (f.state.destroyed = true),
    (f) => (f.deps.tabs.getActiveView = () => null),
  ]) {
    const f = fixture(),
      pending = Promise.withResolvers();
    f.deps.electron.dialog.showMessageBox = () => pending.promise;
    f.service.request(link, f.contents as any);
    change(f);
    pending.resolve({ response: 1 });
    await settle();
    assert.deepEqual(f.opened, []);
  }
});
it('coalesces duplicate attempts and permits a later retry', async () => {
  const f = fixture();
  f.service.request(link, f.contents as any);
  f.service.request(link, f.contents as any);
  await settle();
  assert.equal(f.asked.length, 1);
  f.service.request(link, f.contents as any);
  await settle();
  assert.equal(f.opened.length, 2);
});
it('handles frames and redirects without interfering with ordinary web navigation', async () => {
  const f = fixture();
  f.service.wire(f.contents as any);
  f.service.wire(f.contents as any);
  let prevented = 0;
  f.contents.emit('will-frame-navigate', { url: link, preventDefault: () => prevented++ });
  await settle();
  f.contents.emit('will-redirect', { url: link, preventDefault: () => prevented++ });
  await settle();
  f.contents.emit('will-frame-navigate', { url: 'https://zoom.us/j/123', preventDefault: () => prevented++ });
  assert.equal(prevented, 2);
  assert.equal(f.opened.length, 2);
});
it('reports a missing handler without logging private URLs and permits retry', async () => {
  const f = fixture();
  f.deps.electron.shell.openExternal = async () => {
    throw new Error('private OS failure');
  };
  f.service.request(link, f.contents as any);
  await settle();
  assert.equal(f.asked[1].message, 'Could not open Zoom');
  assert.doesNotMatch(JSON.stringify(f.asked), /private OS failure/);
  f.service.request(link, f.contents as any);
  await settle();
  assert.equal(f.asked.length, 4);
});
it('intercepts named-window meeting links before creating a tab or popup', () => {
  const ctx = mainCtx(),
    calls = [];
  ctx.externalApps.request = (...args) => {
    calls.push(args);
    return true;
  };
  const contents = Object.assign(new EventEmitter(), {
    setWindowOpenHandler(fn) {
      this.openHandler = fn;
    },
  });
  new TabWindows(ctx).wire({ id: 1, view: { webContents: contents } } as any);
  assert.deepEqual(contents.openHandler({ url: link, frameName: 'zoom' }), { action: 'deny' });
  assert.equal(calls[0][0], link);
});
it('intercepts a pasted meeting link before URL normalization or page loading', async () => {
  const ctx = mainCtx(),
    calls = [];
  ctx.tabs = { getActiveView: () => null, navigateActive: () => assert.fail('must not load the custom protocol') };
  ctx.externalApps.request = (raw) => {
    calls.push(raw);
    return true;
  };
  await new NavigationHandlers(ctx).handlers.navigate({} as any, link);
  assert.deepEqual(calls, [link]);
});
