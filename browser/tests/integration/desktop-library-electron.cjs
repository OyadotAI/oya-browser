/** Real Oya shell tabs, native input, both themes, and no real profile data or internal CDP. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const electron = require('electron');
const { app, BrowserWindow, ipcMain } = electron;
const root = path.resolve(__dirname, '../..');
const profile = process.env.OYA_DESKTOP_LIBRARY_PROFILE;
if (!profile) throw Error('Use desktop-library.mjs for disposable profile cleanup');
process.env.OYA_USER_DATA_DIR = profile;
process.env.OYA_AUTO_CONNECT = 'false';
process.env.OYA_LOCAL_MCP = '0';
app.getAppPath = () => root;
const pages = Array.from({ length: 72 }, (_, i) => ({
  title: ['Design notes', 'Typography guide', 'Native APIs', 'Project Atlas'][i % 4] + ` ${i}`,
  url: `https://site${i % 4}.test/page/${i}`,
  time: Date.now() - i * 3600000,
}));
const history = [
  ...pages,
  { title: '<img src=x onerror=alert(1)> Search needle', url: 'https://needle.test/', time: Date.now() },
];
fs.writeFileSync(
  path.join(profile, 'config.json'),
  JSON.stringify({
    ui: { importOffered: true, theme: 'light' },
    imports: [
      { done: true, source: 'Firefox', sites: 12, at: Date.now() },
      { done: true, source: 'Chrome', sites: 8, at: Date.now() - 86400000 },
    ],
    browsingLibrary: { 'persist:oya-browser': { bookmarks: pages, history } },
  }),
);
const handlers = new Map();
const register = ipcMain.handle.bind(ipcMain);
ipcMain.handle = (channel, fn) => {
  handlers.set(channel, fn);
  register(channel, fn);
};
app.on('web-contents-created', (_e, wc) =>
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw Error('Library must not use internal CDP');
    },
  }),
);
const timeout = setTimeout(() => app.exit(1), 60000);
require('../../out/main/index.js');
/** Bounded native readiness, without fixed startup sleeps. */
async function until(read) {
  for (let i = 0; i < 200; i++) {
    const result = await read();
    if (result) return result;
    await new Promise((resolve) => setTimeout(resolve, 40));
  }
  throw Error('Library did not become ready');
}
/** Exact shell bridge retains production IPC authorization. */
const call = (win, name, ...args) =>
  win.webContents.executeJavaScript(`window.oyaBrowser[${JSON.stringify(name)}](...${JSON.stringify(args)})`);
/** Real native pointer input to the packaged React and collection UI. */
async function click(win, selector) {
  const point = await win.webContents.executeJavaScript(`(() => {
    const node = document.querySelector(${JSON.stringify(selector)}); node.scrollIntoView({block:'nearest'});
    const r = node.getBoundingClientRect(); return {x:Math.round(r.x+r.width/2),y:Math.round(r.y+r.height/2)};
  })()`);
  win.webContents.sendInputEvent({ type: 'mouseDown', button: 'left', clickCount: 1, ...point });
  win.webContents.sendInputEvent({ type: 'mouseUp', button: 'left', clickCount: 1, ...point });
}
/** Search operates on the actual tab, including entries beyond its first batch. */
async function search(win, value) {
  await click(win, '.library-page #search');
  win.webContents.selectAll();
  if (value) win.webContents.insertText(value);
  else win.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'Backspace' });
  await until(() =>
    win.webContents.executeJavaScript(
      `document.querySelector('.library-page #search').value === ${JSON.stringify(value)}`,
    ),
  );
}
/** Wait for the requested collection's data, not the initial template. */
const collection = (win, kind) =>
  until(() =>
    win.webContents.executeJavaScript(
      `document.querySelector('.library-page #title')?.textContent === ${JSON.stringify(kind)} && document.querySelectorAll('.library-page .page-row').length > 0`,
    ),
  );
app
  .whenReady()
  .then(async () => {
    const win = await until(() =>
      BrowserWindow.getAllWindows().find((w) => w.webContents.getURL().includes('/out/renderer/index.html')),
    );
    await until(() =>
      win.webContents
        .executeJavaScript('!!window.oyaBrowser && !!document.querySelector("#bookmarks-open")')
        .catch(() => false),
    );
    await call(win, 'enterBrowsing');
    await until(() => win.webContents.executeJavaScript('!!document.querySelector("#launch")?.hidden'));
    win.focus();
    await require('./desktop-downloads.cjs')({ win, call, until, profile });
    const count = BrowserWindow.getAllWindows().length;
    await click(win, '#bookmarks-open');
    await collection(win, 'Bookmarks');
    assert.equal(BrowserWindow.getAllWindows().length, count, 'no dialog or extra window');
    assert.equal(await win.webContents.executeJavaScript('document.querySelector("#start-page").hidden'), true);
    assert.equal(
      await win.webContents.executeJavaScript('document.querySelectorAll(".library-page .page-row").length'),
      50,
    );
    assert.equal(
      await win.webContents.executeJavaScript(
        'getComputedStyle(document.querySelector(".library-page")).backgroundColor',
      ),
      'rgb(248, 249, 250)',
    );
    win.webContents.send('control-state', {
      mode: 'agent',
      connected: true,
      supported: true,
      pageInteractive: true,
      newTabInteractive: true,
    });
    await until(() => win.webContents.executeJavaScript('!document.querySelector(".control-switch").hidden'));
    assert.equal(
      await win.webContents.executeJavaScript(
        'document.querySelector("#bookmarks-open").closest("#bookmarks-bar") !== null',
      ),
      true,
    );
    fs.writeFileSync('/private/tmp/oya-bookmarks-tab-light.png', (await win.webContents.capturePage()).toPNG());
    await search(win, 'atlas 71');
    await until(() =>
      win.webContents.executeJavaScript('document.querySelectorAll(".library-page .page-row").length === 1'),
    );
    await search(win, '');
    await click(win, '.library-page #more');
    await until(() =>
      win.webContents.executeJavaScript('document.querySelectorAll(".library-page .page-row").length === 72'),
    );
    assert.equal(
      await win.webContents.executeJavaScript('document.querySelectorAll(".library-page .page-row").length'),
      72,
    );
    await click(win, '#history-open');
    await collection(win, 'History');
    const ids = await win.webContents.executeJavaScript(
      'Array.from(document.querySelectorAll(".tab-item")).map(n => ({id:Number(n.dataset.id),title:n.textContent}))',
    );
    const historyId = ids.find((t) => t.title.includes('History')).id;
    const bookmarksId = ids.find((t) => t.title.includes('Bookmarks')).id;
    assert.notEqual(historyId, bookmarksId);
    assert.ok(
      await win.webContents.executeJavaScript('document.querySelector(".library-page .day-heading")?.textContent'),
    );
    await search(win, 'needle');
    await until(() =>
      win.webContents.executeJavaScript('document.querySelectorAll(".library-page .page-row").length === 1'),
    );
    assert.equal(await win.webContents.executeJavaScript('document.querySelectorAll(".library-page img").length'), 0);
    await click(win, '.library-page .save-page');
    await until(() =>
      win.webContents.executeJavaScript(
        'document.querySelector(".library-page .save-page").getAttribute("aria-pressed") === "true"',
      ),
    );
    await click(win, '#bookmarks-open');
    await collection(win, 'Bookmarks');
    assert.equal((await call(win, 'readLibrary', bookmarksId)).bookmarks.length, 73);
    assert.equal(await win.webContents.executeJavaScript('document.querySelectorAll(".tab-item").length'), ids.length);
    await call(win, 'setBookmarksBar', false);
    assert.equal(
      await win.webContents.executeJavaScript(
        '["bookmarks-open","history-open"].every(id => document.getElementById(id).getBoundingClientRect().width > 0)',
      ),
      true,
    );
    await assert.rejects(call(win, 'readLibrary', historyId), /no longer/);
    assert.throws(
      () => handlers.get('read-library')({ sender: win.webContents, senderFrame: {} }, bookmarksId),
      /Only the Oya workspace/,
    );
    await call(win, 'saveUiPreferences', { theme: 'dark' });
    win.webContents.reload();
    await until(() => !win.webContents.isLoading());
    await collection(win, 'Bookmarks');
    await until(() => win.webContents.executeJavaScript('document.querySelector("#launch").hidden'));
    await until(() => win.webContents.executeJavaScript('document.documentElement.dataset.theme === "dark"'));
    assert.equal(
      await win.webContents.executeJavaScript(
        'getComputedStyle(document.querySelector(".library-page")).backgroundColor',
      ),
      'rgb(18, 18, 17)',
    );
    fs.writeFileSync('/private/tmp/oya-bookmarks-tab-dark.png', (await win.webContents.capturePage()).toPNG());
    await click(win, '#history-open');
    await collection(win, 'History');
    await until(() => win.webContents.executeJavaScript('document.querySelector("#launch").hidden'));
    fs.writeFileSync('/private/tmp/oya-history-tab-dark.png', (await win.webContents.capturePage()).toPNG());
    await require('./desktop-settings.cjs')({ win, click, until });
    await call(win, 'closeTab', historyId);
    await collection(win, 'Bookmarks');
    await call(win, 'detachTab', bookmarksId);
    const destination = await until(() =>
      BrowserWindow.getAllWindows().find(
        (w) => w !== win && w.webContents.getURL().includes('/out/renderer/index.html'),
      ),
    );
    await collection(destination, 'Bookmarks');
    assert.equal((await call(destination, 'readLibrary', bookmarksId)).kind, 'bookmarks');
    assert.equal(
      destination.getBrowserViews().some((v) => v.webContents.getURL().includes('library')),
      false,
      'private rows never enter an agent-accessible view',
    );
    console.log(
      'Oya collection tabs: search, separate tabs, theme, toolbar buttons, profile isolation and transfer passed',
    );
    clearTimeout(timeout);
    app.exit(0);
  })
  .catch((error) => {
    console.error(error);
    clearTimeout(timeout);
    app.exit(1);
  });
