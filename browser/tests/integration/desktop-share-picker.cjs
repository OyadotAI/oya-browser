/** Native Oya picker QA: real windows, preload IPC and trusted pointer input; no browser automation provider. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { BrowserWindow, BrowserView, session } = require('electron');
const { ShareSheet } = require('../../src/main/screen-share/window.ts');
const { ScreenPicker, ShareIndicator } = require('../../src/main/screen-share/index.ts');

/** Bounded observation of native windows and rendered controls. */
async function until(read) {
  for (let count = 0; count < 100; count++) {
    const value = await read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw Error('Native share picker did not become ready');
}

/** Locate only the browser-owned picker, never the person's normal browser windows. */
async function sheet() {
  return until(() =>
    BrowserWindow.getAllWindows().find(
      (window) => window.isVisible() && window.webContents.getURL().endsWith('/share-picker.html'),
    ),
  );
}

/** A native trusted click drives the isolated preload's ordinary controls. */
async function click(window, selector) {
  const point = await window.webContents.executeJavaScript(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)});
    const rect = node.getBoundingClientRect();
    return { x: Math.round(rect.x + rect.width / 2), y: Math.round(rect.y + rect.height / 2) };
  })()`);
  window.webContents.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, ...point });
  window.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, ...point });
}

/** Select a category and a source, verifying Share is disabled until the explicit choice. */
async function choose(kind, screenshot = false, selector = '.source') {
  const window = await sheet();
  const wc = window.webContents;
  assert.equal(await wc.executeJavaScript('document.getElementById("share").disabled'), true);
  await click(window, `#${kind}`);
  await until(() =>
    wc.executeJavaScript(`document.getElementById(${JSON.stringify(kind)}).getAttribute('aria-selected') === 'true'`),
  );
  await until(() =>
    wc.executeJavaScript(
      `document.querySelectorAll('.preview img').length === document.querySelectorAll('.source').length && [...document.querySelectorAll('.preview img')].every(img => img.complete && img.naturalWidth > 0)`,
    ),
  );
  assert.equal(
    await wc.executeJavaScript('[...document.images].every(img => img.complete && img.naturalWidth > 0)'),
    true,
  );
  await click(window, selector);
  await until(() => wc.executeJavaScript('!document.getElementById("share").disabled'));
  if (screenshot && process.env.OYA_PICKER_SCREENSHOT)
    fs.writeFileSync(process.env.OYA_PICKER_SCREENSHOT, (await wc.capturePage()).toPNG());
  await click(window, '#share');
}

/** Fake OS entries use only pixels from the disposable test page; real desktop enumeration is never called. */
async function installPicker(deps, window) {
  const thumbnail = await window.webContents.capturePage(undefined, { stayHidden: true, stayAwake: true });
  assert.equal(thumbnail.isEmpty(), false, 'fixture must provide real preview pixels');
  const sources = [
    { id: 'window:fixture', name: 'Project notes', thumbnail },
    { id: 'screen:fixture', name: 'Test display', thumbnail },
  ];
  deps.electron.BrowserWindow = BrowserWindow;
  deps.electron.session = session;
  deps.electron.desktopCapturer.getSources = async () => sources;
  deps.appDir = require('node:path').resolve(__dirname, '../..');
  deps.tabs.list = [
    { id: 1, title: 'Project notes', url: window.webContents.getURL(), view: { webContents: window.webContents } },
    ...(await backgroundTabs(window)),
  ];
  deps.screenPicker = new ScreenPicker(deps);
  return sources;
}

/** Real zero-sized detached BrowserViews reproduce restored/background tabs, unlike a BrowserWindow capture. */
async function backgroundTabs(window) {
  const tabs = [];
  for (let i = 0; i < 3; i++) {
    const view = new BrowserView({
      webPreferences: { session: window.webContents.session, sandbox: true, backgroundThrottling: false },
    });
    Object.defineProperty(view.webContents, 'debugger', {
      get() {
        throw Error('Internal CDP is forbidden');
      },
    });
    await view.webContents.loadURL(`https://call.test/background-${i}`);
    assert.equal(view.getBounds().width, 0);
    assert.equal(BrowserWindow.fromWebContents(view.webContents), null);
    tabs.push({ id: i + 2, title: `Background tab ${i + 1}`, url: view.webContents.getURL(), view });
  }
  window.once('closed', () => tabs.forEach((tab) => tab.view.webContents.close()));
  return tabs;
}

/** Verify each category resolves exactly its chosen native source, independently of live display capture. */
async function categories(deps, window, sources) {
  for (const [kind, expected] of [
    ['window', sources[0]],
    ['screen', sources[1]],
  ]) {
    const pending = deps.screenPicker.pick(window.webContents, 'https://call.test', () => true);
    await choose(kind);
    assert.equal((await pending).video, expected);
    for (const tab of deps.tabs.list.slice(1)) {
      assert.equal(tab.view.webContents.isDestroyed(), false, 'preview cleanup must preserve tab renderers');
      assert.equal(BrowserWindow.fromWebContents(tab.view.webContents), null, 'temporary parent must be released');
    }
    assert.equal(BrowserWindow.getAllWindows().length, 1, 'temporary preview surface must be destroyed');
  }
}

/** Every share shows a fresh picker, Escape cancels, and native capture receives only the selected tab. */
async function captureChecks(request, state) {
  const capturing = request();
  await choose('tab', true, '.source:last-child');
  assert.equal(await capturing, 'captured');
  const cancelled = request();
  const window = await sheet();
  assert.equal(await window.webContents.executeJavaScript('document.getElementById("share").disabled'), true);
  window.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'Escape' });
  window.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'Escape' });
  assert.match(await cancelled, /AbortError|NotAllowedError/);
  const revoked = request();
  await sheet();
  state.human = false;
  assert.match(await revoked, /AbortError|NotAllowedError/);
}

/** Denied OS capture remains recoverable from the real picker without losing browser-tab choices. */
async function permissionRecovery(request, deps) {
  const opened = [];
  deps.electron.shell = {
    openExternal: async (url) => {
      opened.push(url);
    },
  };
  const pending = request();
  const window = await sheet();
  await click(window, '#screen');
  await until(() =>
    window.webContents.executeJavaScript('document.getElementById("screen").getAttribute("aria-selected") === "true"'),
  );
  assert.equal(await window.webContents.executeJavaScript('document.querySelectorAll(".source").length'), 0);
  assert.equal(await window.webContents.executeJavaScript('document.getElementById("recovery").hidden'), false);
  await click(window, '#settings');
  assert.equal(await pending, null);
  assert.deepEqual(opened, ['x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture']);
}

/** Invalid image bytes stay as readable placeholders instead of broken-image icons. */
async function brokenPreviews(deps, page) {
  const sources = ['', 'data:image/png;base64,broken'].map((thumbnail, index) => ({
    id: String(index),
    kind: 'tab',
    name: 'Sleeping tab',
    detail: 'call.test',
    thumbnail,
  }));
  const picker = new ShareSheet(
    deps,
    page.webContents,
    {
      origin: 'https://call.test',
      sources,
      desktopError: '',
      canOpenSettings: false,
    },
    () => true,
  );
  const window = await sheet();
  assert.equal(
    await window.webContents.executeJavaScript('document.querySelectorAll(".preview.placeholder").length'),
    2,
  );
  assert.equal(await window.webContents.executeJavaScript('document.querySelectorAll(".preview img").length'), 0);
  await click(window, '#cancel');
  assert.equal(await picker.result, null);
}

/** How soon the ring must clear after the call stops sharing: two polls. */
const SHARE_END_MS = 2000;

/** Until `read` is true or `ms` pass; whether it became true. */
async function within(ms, read) {
  for (const end = Date.now() + ms; Date.now() < end; await new Promise((resolve) => setTimeout(resolve, 50)))
    if (read()) return true;
  return read();
}

/** A real tab share is tracked while live, and Electron's capture count ending it is what clears the ring. */
async function sharingChecks(deps, window) {
  deps.tabs.sendTabList = () => {};
  deps.shell.send = () => {};
  deps.shareIndicator = new ShareIndicator(deps);
  const live = window.webContents.executeJavaScript(
    `navigator.mediaDevices.getDisplayMedia({video:true}).then(s => (window.s = s, 'live'), e => e.name)`,
    true,
  );
  await choose('tab');
  assert.equal(await live, 'live');
  assert.equal(deps.shareIndicator.has(1), true, 'the shared tab is tracked');
  assert.equal(window.webContents.isBeingCaptured(), true, 'a live tab share holds the capture count');
  await window.webContents.executeJavaScript('window.s.getTracks().forEach((t) => t.stop())');
  assert.ok(await within(SHARE_END_MS, () => !deps.shareIndicator.has(1)), 'stopping the share clears it');
}

module.exports = { installPicker, categories, captureChecks, permissionRecovery, brokenPreviews, sharingChecks };
