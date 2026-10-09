/** Real Oya multi-window regression. Owns a disposable profile and localhost pages; no Playwright or live accounts. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createServer } = require('node:http');
const electron = require('electron');
const { app, BrowserWindow, webContents, ipcMain, screen } = electron;
const root = path.resolve(__dirname, '../..');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-windows-test-'));
process.env.OYA_USER_DATA_DIR = profile;
process.env.OYA_AUTO_CONNECT = 'false';
if (!process.env.OYA_WINDOWS_PERSONA_TEST) process.argv.push('--oya-native-browsing');
app.getAppPath = () => root;
fs.writeFileSync(path.join(profile, 'config.json'), JSON.stringify({ ui: { importOffered: true } }));
const handlers = new Map();
const handle = ipcMain.handle.bind(ipcMain);
ipcMain.handle = (channel, listener) => {
  handlers.set(channel, listener);
  handle(channel, listener);
};
const timer = setTimeout(() => {
  console.error('Window regression timed out');
  app.exit(1);
}, 60000);
app.on('quit', () => {
  clearTimeout(timer);
  fs.rmSync(profile, { recursive: true, force: true });
});
const site = createServer((_req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(
    '<!doctype html><title>Transfer fixture</title><input id="draft"><script>window.identity=crypto.randomUUID()</script>',
  );
});
const firstShows = new Map();
app.on('browser-window-created', (_event, win) => {
  const show = win.show.bind(win);
  win.show = () => {
    if (win.webContents.getURL().includes('/out/renderer/index.html') && !firstShows.has(win.id)) {
      firstShows.set(
        win.id,
        win.webContents.executeJavaScript(
          `({ ready: document.documentElement.dataset.ready, tab: !!document.querySelector('.tab-item [aria-selected="true"]'), address: !!document.querySelector('#url-bar')?.getBoundingClientRect().height })`,
        ),
      );
    }
    return show();
  };
});
require('../../out/main/index.js');

/** Poll a bounded local readiness condition, never a website login. */
async function until(read) {
  for (let count = 0; count < 250; count++) {
    const value = await read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 40));
  }
  throw new Error('Local window readiness condition failed');
}
/** Full browser shells, excluding pages and control shields. */
const shells = () =>
  BrowserWindow.getAllWindows().filter((win) => win.webContents.getURL().includes('/out/renderer/index.html'));
/** Invoke the same bridge a person uses, retaining sender-window scoping. */
const call = (win, name, ...args) =>
  win.webContents.executeJavaScript(`window.oyaBrowser[${JSON.stringify(name)}](...${JSON.stringify(args)})`);
/** Wait until the shell preload and renderer are ready. */
async function ready(win) {
  await until(() =>
    win.webContents.executeJavaScript('!!window.oyaBrowser && !!document.querySelector("#root")').catch(() => false),
  );
}
/** Grab a real page by the fixture URL, not by reading any external tab. */
const pageAt = (url) => webContents.getAllWebContents().find((contents) => contents.getURL() === url);

app
  .whenReady()
  .then(async () => {
    await new Promise((resolve) => site.listen(0, '127.0.0.1', resolve));
    const url = `http://127.0.0.1:${site.address().port}/one`;
    const primary = await until(() => shells()[0]);
    await ready(primary);
    await call(primary, 'enterBrowsing');
    await call(primary, 'toggleDevPanel', true);
    primary.webContents.reload();
    await until(() => !primary.webContents.isLoading());
    await until(() =>
      primary.webContents.executeJavaScript('!!document.querySelector("#dev-panel.open")').catch(() => false),
    );
    assert.equal(
      await primary.webContents.executeJavaScript('document.querySelector("#dev-panel").inert'),
      false,
      'visible restored panel must accept input',
    );
    const close = await primary.webContents.executeJavaScript(
      `(() => { const r = document.querySelector('#tools-close').getBoundingClientRect(); return { x: Math.round(r.x+r.width/2), y: Math.round(r.y+r.height/2) }; })()`,
    );
    primary.focus();
    primary.webContents.sendInputEvent({ type: 'mouseDown', ...close, button: 'left', clickCount: 1 });
    primary.webContents.sendInputEvent({ type: 'mouseUp', ...close, button: 'left', clickCount: 1 });
    await until(() => primary.webContents.executeJavaScript('!document.querySelector("#dev-panel.open")'));
    const id = await call(primary, 'newTab', url);
    const page = await until(() => pageAt(url));
    await until(() => !page.isLoading());
    const identity = await page.executeJavaScript(
      'document.querySelector("#draft").value="unsaved draft"; window.identity',
    );
    await page.executeJavaScript(
      'history.pushState({step:1}, "", "#step"); sessionStorage.setItem("transfer", "kept")',
    );
    const entries = page.navigationHistory.getAllEntries();
    const pageId = page.id;
    const listenerCounts = ['did-navigate', 'page-title-updated', 'before-input-event'].map((name) =>
      page.listenerCount(name),
    );
    const originalCursor = screen.getCursorScreenPoint;
    screen.getCursorScreenPoint = () => ({ x: 100, y: 500 });
    await until(() =>
      primary.webContents.executeJavaScript(`!!document.querySelector('.tab-item[data-id="${id}"] .tab-title')`),
    );
    const drag = await primary.webContents.executeJavaScript(
      `(() => { const r = document.querySelector('.tab-item[data-id="${id}"] .tab-title').getBoundingClientRect(); return {x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2)}; })()`,
    );
    primary.focus();
    primary.webContents.focus();
    primary.webContents.sendInputEvent({ type: 'mouseDown', ...drag, button: 'left', clickCount: 1 });
    primary.webContents.sendInputEvent({
      type: 'mouseMove',
      x: drag.x,
      y: drag.y + 130,
      modifiers: ['leftButtonDown'],
    });
    await until(() => primary.webContents.executeJavaScript('!!document.querySelector(".tab-item.dragging")'));
    const preview = await until(() =>
      BrowserWindow.getAllWindows().find(
        (win) => win.webContents.getURL().includes('/tab-preview/index.html') && win.isVisible(),
      ),
    );
    assert.equal(preview.isFocusable(), false);
    assert.equal(primary.isFocused(), true);
    assert.ok(primary.getBrowserViews().some((view) => view.webContents === page));
    await until(() => preview.webContents.executeJavaScript('document.getElementById("thumbnail").naturalWidth > 0'));
    fs.writeFileSync(path.join(os.tmpdir(), 'oya-tab-preview.png'), (await preview.webContents.capturePage()).toPNG());
    primary.webContents.sendInputEvent({ type: 'mouseUp', x: drag.x, y: drag.y + 130, button: 'left', clickCount: 1 });
    const secondary = await until(() => shells().find((win) => win !== primary));
    await until(() => secondary.isVisible());
    await ready(secondary);
    await until(() => preview.isDestroyed());
    assert.deepEqual(await firstShows.get(secondary.id), { ready: 'true', tab: true, address: true });
    assert.equal(
      await secondary.webContents.executeJavaScript(
        `!!document.querySelector('.tab-item[data-id="${id}"] [aria-selected="true"]') && document.querySelector('#url-bar').getBoundingClientRect().height > 0`,
      ),
      true,
    );
    assert.equal(page.id, pageId);
    assert.ok(secondary.getBrowserViews().some((view) => view.webContents === page));
    assert.ok(!primary.getBrowserViews().some((view) => view.webContents === page));
    assert.deepEqual(
      await page.executeJavaScript(
        '[window.identity, document.querySelector("#draft").value, sessionStorage.getItem("transfer")]',
      ),
      [identity, 'unsaved draft', 'kept'],
    );
    assert.deepEqual(page.navigationHistory.getAllEntries(), entries);
    assert.deepEqual(
      ['did-navigate', 'page-title-updated', 'before-input-event'].map((name) => page.listenerCount(name)),
      listenerCounts,
    );
    assert.equal(await page.executeJavaScript('typeof window.oyaBrowser'), 'undefined');
    assert.throws(
      () => handlers.get('new-window')({ sender: page, senderFrame: page.mainFrame }),
      /Only the Oya workspace/,
    );
    await assert.rejects(call(primary, 'detachTab', id), /Tab is not in this window/);
    await page.executeJavaScript('document.title="Moved page title"');
    await until(() =>
      secondary.webContents.executeJavaScript('document.body.textContent.includes("Moved page title")'),
    );
    primary.focus();
    await until(() => primary.isFocused());
    const bounds = primary.getContentBounds();
    screen.getCursorScreenPoint = () => ({ x: bounds.x + 300, y: bounds.y + 20 });
    await call(secondary, 'detachTab', id);
    await until(() => secondary.isDestroyed());
    assert.equal(BrowserWindow.getAllWindows().length, 1);
    assert.ok(primary.getBrowserViews().some((view) => view.webContents === page));
    assert.equal(await page.executeJavaScript('document.querySelector("#draft").value'), 'unsaved draft');
    await call(primary, 'beginTabDrag', id);
    await call(primary, 'endTabDrag');
    await until(() => BrowserWindow.getAllWindows().length === 1);
    assert.ok(primary.getBrowserViews().some((view) => view.webContents === page));
    primary.focus();
    primary.webContents.sendInputEvent({
      type: 'keyDown',
      keyCode: 'N',
      modifiers: [process.platform === 'darwin' ? 'meta' : 'control'],
    });
    primary.webContents.sendInputEvent({
      type: 'keyUp',
      keyCode: 'N',
      modifiers: [process.platform === 'darwin' ? 'meta' : 'control'],
    });
    const third = await until(() => shells().find((win) => win !== primary));
    await until(() => third.isVisible());
    await ready(third);
    assert.deepEqual(await firstShows.get(third.id), { ready: 'true', tab: true, address: true });
    const otherId = await call(third, 'newTab', url.replace('/one', '/two'));
    assert.notEqual(otherId, id);
    const other = await until(() => pageAt(url.replace('/one', '/two')));
    await until(() => !other.isLoading());
    third.close();
    await until(() => third.isDestroyed());
    assert.equal(page.isDestroyed(), false);
    assert.equal(other.isDestroyed(), true);
    screen.getCursorScreenPoint = originalCursor;
    console.log(
      'PASS: pointer tear-off, Cmd/Ctrl+N, live page/form/history transfer, return transfer, independent close, globally unique ids, renderer isolation, scoped IPC',
    );
  })
  .then(
    () => {
      site.close();
      app.exit(0);
    },
    (error) => {
      console.error(error);
      site.close();
      app.exit(1);
    },
  );
