/** Real Oya permission callbacks with fake devices and OS decisions; no network, user profile, or debugger access. */
const assert = require('node:assert/strict');
const path = require('node:path');
const { app, BrowserWindow, session, webContents } = require('electron');
const { MediaPermissions } = require('../../src/main/app/media-permissions.ts');
const { ScreenShare } = require('../../src/main/app/screen-share.ts');
const { SitePermissionStore } = require('../../src/main/app/site-permissions.ts');
const { JsonFile } = require('../../src/main/app/json-file.ts');
const { installPermissions } = require('../../src/main/identity/permissions.ts');
const {
  installPicker,
  categories,
  captureChecks,
  permissionRecovery,
  brokenPreviews,
  sharingChecks,
} = require('./desktop-share-picker.cjs');

const profile = process.env.OYA_DESKTOP_MEDIA_PROFILE;
if (!profile) throw new Error('Run desktop-media.mjs so the parent owns profile cleanup');
app.setPath('userData', profile);
app.commandLine.appendSwitch('use-fake-device-for-media-stream');
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => app.exit(1), 30000);

/** Exercise getUserMedia itself: successful test streams are always stopped immediately. */
function capture(contents, constraints) {
  return contents.executeJavaScript(`(async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia(${JSON.stringify(constraints)});
      const kinds = stream.getTracks().map(track => track.kind);
      stream.getTracks().forEach(track => track.stop());
      return { kinds };
    } catch (error) { return { error: error.name }; }
  })()`);
}

/** Wire production services around a real tab, substituting only the native user and OS decisions. */
function permissions(window, state) {
  const prompts = [];
  const deps = {
    electron: {
      dialog: {
        showMessageBox: async (_window, options) => (prompts.push(options), { response: state.response ?? 1 }),
      },
      systemPreferences: {
        askForMediaAccess: async () => state.os,
        getMediaAccessStatus: () => (state.os ? 'granted' : 'denied'),
      },
    },
    shell: { window },
    tabs: { activeTabId: 1, getActiveView: () => (window.isDestroyed() ? null : { webContents: window.webContents }) },
    agentTab: { humanMay: () => state.human },
    governance: { configuration: null },
    persona: { partitionName: () => 'persist:media-test' },
    sitePermissions: new SitePermissionStore(new JsonFile(() => path.join(profile, 'site-permissions.json'))),
  };
  const media = new MediaPermissions(deps, 'darwin');
  return { media, deps, prompts };
}

/** A real getDisplayMedia request must reach Oya's consent gate before any OS capture begins. */
async function screenPermission(ses, window, state) {
  state.human = true;
  state.os = false;
  const f = permissions(window, state);
  const opened = [];
  f.deps.electron.shell = {
    openExternal: async (url) => {
      opened.push(url);
    },
  };
  f.deps.electron.webContents = webContents;
  f.deps.electron.desktopCapturer = {
    getSources: async () => {
      throw new Error('OS refusal must prevent capture');
    },
  };
  f.deps.mediaPermissions = f.media;
  const screen = new ScreenShare(f.deps);
  const trace = [];
  const hooks = {
    setPermissionCheckHandler: (handler) =>
      ses.setPermissionCheckHandler((...args) => {
        const answer = handler(...args);
        trace.push(['check', args[1], answer]);
        return answer;
      }),
    setPermissionRequestHandler: (handler) =>
      ses.setPermissionRequestHandler((...args) => {
        trace.push(['request', args[1], args[3]]);
        return handler(...args);
      }),
    setDisplayMediaRequestHandler: (handler) =>
      ses.setDisplayMediaRequestHandler((request, callback) => {
        trace.push(['display', request.securityOrigin, request.frame === window.webContents.mainFrame]);
        return handler(request, callback);
      }),
  };
  installPermissions(hooks, undefined, f.media, screen);
  const sources = await installPicker(f.deps, window);
  const enumerate = f.deps.electron.desktopCapturer.getSources;
  let enumerations = 0;
  f.deps.electron.desktopCapturer.getSources = async () => {
    enumerations++;
    return enumerate();
  };
  const legacy = await capture(window.webContents, {
    video: { mandatory: { chromeMediaSource: 'desktop', chromeMediaSourceId: window.getMediaSourceId() } },
  });
  assert.ok(legacy.error, 'legacy desktop constraints cannot bypass OS refusal');
  const request = () =>
    window.webContents.executeJavaScript(
      `navigator.mediaDevices.getDisplayMedia({video:true}).then(
    async stream => {
      try {
        const video = document.createElement('video');
        video.muted = true;
        video.srcObject = stream;
        await video.play();
        return video.videoWidth > 0 && video.videoHeight > 0 ? 'captured' : 'empty capture';
      } finally { stream.getTracks().forEach(track => track.stop()); }
    },
    error => error.name + ': ' + error.message
  )`,
      true,
    );
  assert.match(await request(), /AbortError|NotAllowedError/);
  assert.deepEqual(opened, ['x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture']);
  assert.equal(enumerations, 2, 'macOS must be asked for sources even when its initial status is denied');
  assert.ok(
    f.prompts.some((prompt) => /Share your screen/.test(prompt.message)),
    JSON.stringify(trace),
  );
  assert.equal(
    f.prompts.some((prompt) => /Device access is blocked/.test(prompt.message)),
    false,
  );
  await permissionRecovery(
    () => f.deps.screenPicker.pick(window.webContents, 'https://call.test', () => state.human),
    f.deps,
  );
  state.os = true;
  await brokenPreviews(f.deps, window);
  await categories(f.deps, window, sources);
  await captureChecks(request, state);
  state.human = true;
  await sharingChecks(f.deps, window);
  assert.ok(
    trace.some(([kind]) => kind === 'display'),
    'the native display-media handler ran',
  );
  assert.equal(f.prompts.filter((prompt) => /Share your screen/.test(prompt.message)).length, 1);
  state.human = false;
  assert.match(await request(), /NotAllowedError/);
}

/** A saved Allow survives an OS refusal, page reload, and service reconstruction without another site prompt. */
async function run() {
  await app.whenReady();
  const ses = session.fromPartition('persist:media-test');
  ses.protocol.handle(
    'https',
    () =>
      new Response(
        '<!doctype html><title>Project notes</title><style>body{background:#f4f2ec;color:#293728;font:20px system-ui;padding:48px}small{color:#64745e}h1{font-size:40px}</style><small>WORKSPACE / PRODUCT</small><h1>Project notes</h1><p>A clear view of what comes next.</p><hr><p>01 &nbsp; Research and planning</p><p>02 &nbsp; Design review</p><p>03 &nbsp; Ready to share</p>',
        { headers: { 'content-type': 'text/html; charset=utf-8' } },
      ),
  );
  const window = new BrowserWindow({ show: false, webPreferences: { session: ses, sandbox: true } });
  const contents = window.webContents;
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw new Error('Internal CDP is forbidden');
    },
  });
  const state = { os: false, human: true };
  const f = permissions(window, state);
  installPermissions(ses, undefined, f.media);
  await window.loadURL('https://call.test/first');
  assert.deepEqual(await capture(contents, { audio: true }), { error: 'NotAllowedError' });
  assert.equal(f.deps.sitePermissions.has('persist:media-test', 'https://call.test', 'audio'), true);
  assert.deepEqual(await capture(contents, { audio: true }), { error: 'NotAllowedError' });
  assert.equal(f.prompts.filter((prompt) => prompt.message === 'Allow microphone?').length, 1);
  state.os = true;
  assert.deepEqual(await capture(contents, { audio: true }), { kinds: ['audio'] });
  await window.loadURL('https://call.test/second');
  const reopened = permissions(window, state);
  installPermissions(ses, undefined, reopened.media);
  assert.deepEqual(await capture(contents, { audio: true }), { kinds: ['audio'] });
  assert.deepEqual(reopened.prompts, [], 'the persisted site decision needs no new dialog');
  assert.deepEqual(await capture(contents, { video: true }), { kinds: ['video'] });
  assert.equal(reopened.prompts[0].message, 'Allow camera?');
  state.human = false;
  assert.deepEqual(await capture(contents, { audio: true }), { error: 'NotAllowedError' });
  await screenPermission(ses, window, state);
  window.destroy();
  console.log(
    'Oya desktop media passed: native callbacks, remembered consent, OS refusal/recovery, reload, camera, display-media handoff and agent refusal.',
  );
}

run().then(
  () => {
    clearTimeout(deadline);
    app.exit(0);
  },
  (error) => {
    console.error(error);
    clearTimeout(deadline);
    app.exit(1);
  },
);
