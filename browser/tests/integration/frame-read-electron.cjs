/**
 * Cross-origin iframes on stock Electron (the desktop app): the agent's page read lists
 * a form served from another origin, the page cannot see the analyzer that read it, and
 * typing then clicking through the real driver submits that form. Debugger access is
 * forbidden throughout, as it is in the app.
 */
const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow, ipcMain, webContents } = require('electron');
const { World, useFramePages } = require('../../src/main/native/index.ts');
const { useStockWorld } = require('../../src/main/native/stock-world.ts');
const { STOCK_PERSONA } = require('../../src/shared/stock-persona.ts');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { actionScript } = require('../../src/main/actions/scripts.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');

const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-frame-read-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Frame read test timed out');
  app.exit(1);
}, 60000);
const world = new World({
  analyzerScript: fs.readFileSync(path.resolve(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'frame-read',
});
let submitted = null;

/** Page A embeds a form from origin B (localhost vs 127.0.0.1); B records what it was sent. */
const server = http.createServer((req, res) => {
  const port = server.address().port;
  res.setHeader('Content-Type', 'text/html');
  if (req.url.startsWith('/done')) {
    submitted = new URL(req.url, 'http://x').searchParams.get('name');
    return res.end('<!doctype html><p>Thanks</p>');
  }
  res.end(
    req.url === '/parent'
      ? `<!doctype html><h1>Portal</h1><iframe src="http://localhost:${port}/form" style="width:600px;height:300px;border:0"></iframe>`
      : '<!doctype html><form action="/done"><label>Name <input name="name"></label><button>Create</button></form>',
  );
});

/** Runs the read and the actions. */
async function run() {
  await new Promise((resolve) => server.listen(0, resolve));
  ipcMain.on(STOCK_PERSONA.CHANNEL, (event) => (event.returnValue = ''));
  useStockWorld(ipcMain);
  useFramePages((frame) => webContents.fromFrame(frame));
  const win = new BrowserWindow({
    width: 900,
    height: 600,
    // Hidden: frame input runs inside the frame, so the test needs no focus and cannot catch a person's typing.
    show: false,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegrationInSubFrames: true,
      additionalArguments: [STOCK_PERSONA.ARGUMENT],
      preload: path.resolve(__dirname, '../../out/preload/recording.js'),
    },
  });
  Object.defineProperty(win.webContents, 'debugger', {
    get() {
      throw new Error('Internal CDP is forbidden');
    },
  });
  await win.loadURL(`http://127.0.0.1:${server.address().port}/parent`);
  await new Promise((resolve) => setTimeout(resolve, 1000));
  const view = { webContents: win.webContents };
  const frame = win.webContents.mainFrame.frames[0];
  assert.notEqual(frame.processId, win.webContents.mainFrame.processId, 'the form must be a cross-process frame');
  const results = [];
  const driver = new PageDriver({
    mouse: new Mouse(),
    keyboard: new Keyboard(process.platform),
    getActiveView: () => view,
    injectScripts: (v) => world.ensure(v),
    worldEval: (v, code, f) => world.evaluate(v, code, { frame: f }),
    sendResult: (_id, ok, data, error) => results.push({ ok, data, error }),
    analysisStarted: () => {},
    analysisFinished: () => {},
    config: { values: { ui: { pageFormat: 'markdown' } } },
  });
  await driver.runInjected('read', 'analyze', {}, view);
  const read = results.pop();
  assert.equal(read.ok, true, read.error);
  const page = String(read.data?.page ?? read.data?.markdown ?? JSON.stringify(read.data));
  assert.match(page, /Create/, 'the form inside the frame is read, under its [iframe] heading');
  const raw = await world.evaluate(view, actionScript('analyze', {}));
  const merged = await require('../../src/main/actions/frame-scope.ts').withFrames(
    driver,
    view,
    raw,
    actionScript('analyze', {}),
  );
  const input = merged.data.elements.find((e) => e.type === 'input' || /name/i.test(e.text || ''));
  const button = merged.data.elements.find((e) => /create/i.test(e.text || ''));
  assert.ok(input && button, 'the read lists the form’s input and button: ' + JSON.stringify(merged.data.elements));
  assert.equal(await frame.executeJavaScript('typeof analyzePage'), 'undefined', 'the page cannot see the analyzer');
  await driver.runPageAction('type', 'type', { selector: `[data-ac-id="${input.id}"]`, text: 'Ada' }, view);
  assert.equal(results.pop()?.ok, true, 'typing into the frame works');
  await driver.runPageAction('click', 'click', { selector: `[data-ac-id="${button.id}"]` }, view);
  const clicked = results.pop();
  assert.equal(clicked?.ok, true, 'clicking in the frame works: ' + JSON.stringify(clicked));
  for (let i = 0; i < 50 && submitted === null; i++) await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal(submitted, 'Ada', 'type then click submitted the cross-origin form');
}

app
  .whenReady()
  .then(run)
  .then(
    () => {
      clearTimeout(deadline);
      console.log('frame read: ok');
      app.exit(0);
    },
    (error) => {
      console.error(error);
      app.exit(1);
    },
  );
