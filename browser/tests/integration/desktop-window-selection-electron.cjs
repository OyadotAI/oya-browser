/** Actual Oya sheet/preload/IPC with synthetic OS selection; no desktop pixels, website, provider or debugger. */
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { setTimeout: pause } = require('node:timers/promises');
const electron = require('electron');
const { app, BrowserWindow } = electron;
const { ShareSheet } = require('../../src/main/screen-share/window.ts');
const { NativeWindowChoice } = require('../../src/main/screen-share/native-window-choice.ts');
const { NativeWindowSelector } = require('../../src/main/screen-share/native-window-selector.ts');
const profile = process.env.OYA_WINDOW_SELECTION_PROFILE;
if (!profile) throw Error('Run desktop-window-selection.mjs');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
app.on('web-contents-created', (_event, contents) =>
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw Error('Internal debugger access is forbidden');
    },
  }),
);
const deadline = setTimeout(() => {
  console.error('Window selection fixture timed out');
  app.exit(1);
}, 60000);
/** Bounded readiness from actual native windows and production preload state. */
async function until(read, message) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (await read()) return;
    await pause(20);
  }
  throw Error(message);
}
/** Trusted native input exercises the ordinary isolated preload handlers. */
async function click(window, selector) {
  const point = await window.webContents.executeJavaScript(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)}); node.scrollIntoView({block:'nearest'});
    const r = node.getBoundingClientRect(); return {x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2)};
  })()`);
  window.webContents.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, ...point });
  window.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, ...point });
}
/** Build a production private sheet, substituting only the OS callback and native source enumeration. */
async function fixture(parent) {
  const state = { callback: null, current: true, native: true, excluded: [], enumerations: 0 };
  const native = {
    windowId: Number(parent.getMediaSourceId().split(':')[1]),
    processId: process.pid,
    processStarted: 1,
  };
  const source = {
    id: parent.getMediaSourceId(),
    name: 'Project notes — '.repeat(18),
    thumbnail: await parent.webContents.capturePage(),
  };
  const adapter = {
    ready: () => !state.callback,
    start: (callback, excluded) => {
      state.callback = callback;
      state.excluded = excluded;
      return true;
    },
    cancel() {},
    current: () => state.native,
  };
  const selector = new NativeWindowSelector(profile, { platform: 'darwin', version: '15.2', load: () => adapter });
  const api = {
    ...electron,
    desktopCapturer: {
      getSources: async () => {
        state.enumerations++;
        return [source];
      },
    },
    systemPreferences: { getMediaAccessStatus: () => 'granted' },
  };
  const choice = new NativeWindowChoice(api, selector, () => state.current);
  const data = {
    origin: 'https://meet.an-explicitly-long-synthetic-recipient.example:8443',
    sources: [],
    desktopError: '',
    canOpenSettings: false,
    native: 'ready',
  };
  const sheet = new ShareSheet(
    { electron: api, appDir: path.resolve(__dirname, '../..'), native: { selector, choice } },
    parent.webContents,
    data,
    () => state.current,
  );
  const window = BrowserWindow.getAllWindows().find((win) => win !== parent);
  await until(() => window.isVisible(), 'production sheet visible');
  const read = (code) => window.webContents.executeJavaScript(code);
  const finish = (result) => {
    const callback = state.callback;
    state.callback = null;
    callback(result);
  };
  return { state, native, source, selector, choice, sheet, window, read, finish };
}
/** Window selection stays unconfirmed until the second Oya action, at narrow and normal widths. */
async function selection(parent, width, height) {
  const f = await fixture(parent);
  f.window.setContentSize(width, height);
  await click(f.window, '#window');
  await until(() => f.read('!document.getElementById("native-entry").hidden'), 'direct action visible');
  await click(f.window, '#native-start');
  await until(() => !!f.state.callback, 'OS callback admitted');
  assert.equal(f.window.isVisible(), false);
  assert.deepEqual(f.state.excluded, [Number(f.window.getMediaSourceId().split(':')[1])]);
  assert.equal(f.state.enumerations, 0);
  f.finish({ status: 'selected', ...f.native });
  await until(
    () => f.read('document.activeElement.id === "cancel" && !document.getElementById("native-candidate").hidden'),
    'candidate with Cancel focused',
  );
  assert.equal(f.state.enumerations, 1);
  assert.equal(await f.read('document.getElementById("share").textContent'), 'Select and share');
  await geometry(f, width, height);
  await click(f.window, '#native-list');
  await until(() => f.read('document.getElementById("native-candidate").hidden'), 'list returns');
  assert.equal(await f.read('document.getElementById("share").disabled'), true);
  await click(f.window, '#native-start');
  await until(() => !!f.state.callback, 'second OS selection');
  f.finish({ status: 'selected', ...f.native });
  await until(() => f.read('!document.getElementById("native-candidate").hidden'), 'second candidate');
  await click(f.window, '#share');
  const token = await f.sheet.result;
  assert.equal(f.state.enumerations, 3);
  assert.equal(f.choice.take(token), f.source);
  f.choice.retire();
}
/** Measure geometry and retain only screenshots of disposable fixture content. */
async function geometry(f, width, height) {
  await pause(80);
  const bounds = await f.read(`(() => { const footer=document.querySelector('footer').getBoundingClientRect();
    return {overflow:document.documentElement.scrollWidth>innerWidth,bottom:footer.bottom,buttons:[...document.querySelectorAll('footer button')].map(b=>{const r=b.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth&&r.bottom<=innerHeight})}; })()`);
  assert.equal(bounds.overflow, false);
  assert.ok(bounds.bottom <= height && bounds.buttons.every(Boolean));
  if (process.env.OYA_WINDOW_SELECTION_SHOTS) {
    fs.mkdirSync(process.env.OYA_WINDOW_SELECTION_SHOTS, { recursive: true });
    fs.writeFileSync(
      path.join(process.env.OYA_WINDOW_SELECTION_SHOTS, `confirmation-${width}.png`),
      (await f.window.webContents.capturePage()).toPNG(),
    );
  }
}
/** A vanished target offers deliberate list recovery, and native cancellation ends the whole attempt. */
async function failures(parent) {
  const f = await fixture(parent);
  await click(f.window, '#window');
  await until(() => f.read('!document.getElementById("native-entry").hidden'), 'failure action visible');
  await click(f.window, '#native-start');
  await until(() => !!f.state.callback, 'failure selection');
  f.finish({ status: 'selected', ...f.native });
  await until(() => f.read('!document.getElementById("native-candidate").hidden'), 'failure candidate');
  f.state.native = false;
  await click(f.window, '#share');
  await until(
    () => f.read('document.getElementById("native-status").textContent.includes("no longer available")'),
    'closed target explanation',
  );
  assert.equal(await f.read('document.getElementById("share").disabled'), true);
  await click(f.window, '#native-start');
  await until(() => !!f.state.callback, 'cancel selection');
  f.finish({ status: 'cancelled' });
  assert.equal(await f.sheet.result, null);
  f.choice.retire();
}
/** Use a real native Oya parent and private picker; never start external desktop capture. */
async function main() {
  await app.whenReady();
  const parent = new BrowserWindow({ show: true, width: 700, height: 500, webPreferences: { sandbox: true } });
  await parent.loadURL('data:text/html,<title>Owned fixture</title><h1>Project notes</h1>');
  for (const [width, height] of [
    [360, 400],
    [600, 400],
    [780, 620],
  ])
    await selection(parent, width, height);
  await failures(parent);
  parent.destroy();
  clearTimeout(deadline);
  console.log(
    'PASS native Oya window confirmation: 360/600/780 layouts, exact IPC, explicit final consent, fresh resolution, back-to-list, source closure and native cancellation. Apple picker input remains synthetic.',
  );
  app.exit(0);
}
main().catch((error) => {
  console.error(error);
  clearTimeout(deadline);
  app.exit(1);
});
