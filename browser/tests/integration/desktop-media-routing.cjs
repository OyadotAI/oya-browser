/** Meet-style combined devices and screen sharing use the requester's window through the production router. */
const assert = require('node:assert/strict');
const { BrowserWindow, webContents } = require('electron');
const { WindowServiceRouter } = require('../../src/main/windows/service-router.ts');
const { ScreenShare } = require('../../src/main/app/screen-share.ts');
const { installPermissions } = require('../../src/main/identity/permissions.ts');
const { installPicker, captureChecks } = require('./desktop-share-picker.cjs');
const { buildMaskPreamble, buildPermissionsBody } = require('../../anonymity/stealth.js');

/** Concrete window-local collaborators let the production router select a different application target. */
function scope(window, tabs) {
  return { shell: { window }, tabs, shortcuts: {}, shield: {}, layout: {}, overlays: {}, findBar: {} };
}
/** Query native permission state without storing page content, device identifiers or account data. */
function permissionStates(contents) {
  return contents.executeJavaScript(
    `Promise.all(['microphone', 'camera'].map(async name => ({name, state: (await navigator.permissions.query({name})).state})))`,
  );
}
/** Fake devices still yield real usable native MediaStream tracks. */
function combinedCapture(contents) {
  return contents.executeJavaScript(
    `(async () => {
    const stream = await navigator.mediaDevices.getUserMedia({audio:true, video:true});
    try {
      const video = document.createElement('video'); video.muted = true; video.srcObject = stream;
      await video.play();
      return {kinds: stream.getTracks().map(track => track.kind).sort(), live: stream.getTracks().every(track => track.readyState === 'live'), pixels: video.videoWidth > 0 && video.videoHeight > 0};
    } finally { stream.getTracks().forEach(track => track.stop()); }
  })()`,
    true,
  );
}
/** Source choice remains explicit even though only the safe disposable tab can provide test pixels. */
function displayCapture(contents) {
  return contents.executeJavaScript(
    `navigator.mediaDevices.getDisplayMedia({video:true}).then(async stream => {
    try { const video = document.createElement('video'); video.muted = true; video.srcObject = stream; await video.play(); return video.videoWidth > 0 ? 'captured' : 'empty capture'; }
    finally { stream.getTracks().forEach(track => track.stop()); }
  }, error => error.name)`,
    true,
  );
}
/** Run real callbacks with two native owner windows, retaining fake OS consent and fake hardware only. */
async function windowRouting(ses, window, fixture) {
  const other = new BrowserWindow({ show: false, webPreferences: { session: ses, sandbox: true } });
  const state = { human: true, os: true };
  const f = fixture(window, state);
  const deps = f.deps;
  deps.mediaPermissions = f.media;
  deps.electron.webContents = webContents;
  deps.electron.desktopCapturer = {};
  await installPicker(deps, window);
  const owner = scope(window, deps.tabs);
  const target = scope(other, { activeTabId: 99, list: [], getActiveView: () => ({ webContents: other.webContents }) });
  const scopes = [owner, target];
  deps.windows = {
    current: target,
    allTabs: () => scopes.flatMap((scope) => scope.tabs.list),
    ownerOfContents: (contents) =>
      scopes.find((scope) => scope.tabs.list.some((tab) => tab.view.webContents === contents)),
    each: (run) => scopes.forEach(run),
  };
  new WindowServiceRouter(deps, deps.windows).install();
  const parents = [];
  deps.electron.dialog.showMessageBox = async (parent) => {
    parents.push(parent);
    return { response: 1 };
  };
  const trace = [];
  const hooks = {
    setPermissionCheckHandler: (handler) =>
      ses.setPermissionCheckHandler((...args) => {
        const result = handler(...args);
        trace.push({ permission: args[1], mediaType: args[3]?.mediaType, result });
        return result;
      }),
    setPermissionRequestHandler: (handler) => ses.setPermissionRequestHandler(handler),
    setDisplayMediaRequestHandler: (handler) => ses.setDisplayMediaRequestHandler(handler),
  };
  const screen = new ScreenShare(deps, 'darwin');
  installPermissions(hooks, undefined, f.media, screen);
  try {
    await window.loadURL('https://meet-routing.test/prejoin');
    await window.webContents.executeJavaScript(`(() => { ${buildMaskPreamble()}\n${buildPermissionsBody()} })()`);
    const before = await permissionStates(window.webContents);
    assert.deepEqual(
      before.map((permission) => permission.state),
      ['prompt', 'prompt'],
    );
    assert.deepEqual(await combinedCapture(window.webContents), {
      kinds: ['audio', 'video'],
      live: true,
      pixels: true,
    });
    const after = await permissionStates(window.webContents);
    assert.deepEqual(
      after.map((permission) => permission.state),
      ['granted', 'granted'],
    );
    assert.equal(parents[0], window, 'combined-device consent belongs to the requester, not the global target');
    assert.equal(deps.windows.current, target, 'permission must not steal the agent/application target');
    await captureChecks(() => displayCapture(window.webContents), state);
    console.log(JSON.stringify({ meetingPermissionQuery: { before, after, trace } }));
    console.log(
      'Oya requester-window media passed: combined microphone/camera tracks, actual video pixels, explicit tab share, cancel and takeover refusal.',
    );
  } finally {
    other.destroy();
  }
}
module.exports = { windowRouting };
