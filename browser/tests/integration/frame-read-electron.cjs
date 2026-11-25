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
const { useDebugChannel } = require('../../src/main/browser-debug/index.ts');
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

/** A form that sends `who` with the typed name, so a test can tell which frame submitted. */
const form = (who) =>
  `<!doctype html><form action="/done"><input type="hidden" name="who" value="${who}"><label>Name <input name="name"></label><button>Create ${who}</button></form>`;

/** The pages, by path; `a` is the page's origin (127.0.0.1), `b` and `c` two other origins (localhost on two ports). */
const pages = (b, c) => ({
  '/parent': `<!doctype html><h1>Portal</h1><iframe src="${b}/form" style="width:600px;height:300px;border:0"></iframe>`,
  '/form': form('single'),
  '/nested': `<!doctype html><h1>Shop</h1><iframe src="${b}/middle" style="width:700px;height:400px;border:0"></iframe>`,
  '/middle': `<!doctype html><h2>Checkout</h2><iframe src="${c}/card" style="width:600px;height:250px;border:0"></iframe>`,
  '/card': form('nested'),
  '/siblings': `<!doctype html><iframe src="${b}/one" style="width:400px;height:200px"></iframe><iframe src="${b}/two" style="width:400px;height:200px"></iframe>`,
  '/one': form('one'),
  '/two': form('two'),
});

/** One handler for both servers; it records what each form sent. */
function handle(req, res) {
  res.setHeader('Content-Type', 'text/html');
  const url = new URL(req.url, 'http://x');
  if (url.pathname === '/done') {
    submitted = `${url.searchParams.get('who')}:${url.searchParams.get('name')}`;
    return res.end('<!doctype html><p>Thanks</p>');
  }
  res.end(
    pages(`http://localhost:${server.address().port}`, `http://localhost:${second.address().port}`)[url.pathname] ?? '',
  );
}
const server = http.createServer(handle);
const second = http.createServer(handle);

/** Runs the read and the actions. */
async function run() {
  await new Promise((resolve) => server.listen(0, resolve));
  await new Promise((resolve) => second.listen(0, resolve));
  ipcMain.on(STOCK_PERSONA.CHANNEL, (event) => (event.returnValue = ''));
  useStockWorld(ipcMain);
  useDebugChannel(ipcMain); // as main.ts does: frames are found and reached through their exact owners
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
  assert.equal(submitted, 'single:Ada', 'type then click submitted the cross-origin form');
  await nestedAndSiblings(win, driver, results, view);
}

/** Loads `path`, reads it the way analyze_page does, and answers the merged elements. */
async function readPage(win, driver, view, path) {
  await win.loadURL(`http://127.0.0.1:${server.address().port}${path}`);
  await new Promise((resolve) => setTimeout(resolve, 1000));
  const script = actionScript('analyze', {});
  const merged = await require('../../src/main/actions/frame-scope.ts').withFrames(
    driver,
    view,
    await world.evaluate(view, script),
    script,
  );
  return merged.data.elements;
}

/** Types `name` into the form whose button says `Create ${who}`, presses it, and answers what was submitted. */
async function submit(driver, results, view, elements, who, name) {
  const button = elements.find((e) => e.text === `Create ${who}`);
  const input = elements.find(
    (e) => e.type === 'input' && Math.floor(e.id / 100000) === Math.floor(button.id / 100000),
  );
  assert.ok(input && button, `the read lists the ${who} form: ` + JSON.stringify(elements));
  submitted = null;
  await driver.runPageAction('type', 'type', { selector: `[data-ac-id="${input.id}"]`, text: name }, view);
  assert.equal(results.pop()?.ok, true, `typing into the ${who} frame works`);
  await driver.runPageAction('click', 'click', { selector: `[data-ac-id="${button.id}"]` }, view);
  const clicked = results.pop();
  assert.equal(clicked?.ok, true, `clicking in the ${who} frame works: ` + JSON.stringify(clicked));
  for (let i = 0; i < 50 && submitted === null; i++) await new Promise((resolve) => setTimeout(resolve, 100));
  return submitted;
}

/**
 * A field two frames deep, clicked, then typed into and submitted by keys that name no element,
 * in a hidden window: an agent drives Oya in the background, where no document has window focus.
 */
async function typeByFocus(win, driver, results, view) {
  const elements = await readPage(win, driver, view, '/nested');
  const input = elements.find((e) => e.type === 'input' && e.id >= 100000);
  const done = (r) => assert.equal(r?.ok, true, JSON.stringify(r));
  submitted = null;
  await driver.runPageAction('click', 'click', { selector: `[data-ac-id="${input.id}"]` }, view);
  done(results.pop());
  await driver.runPageAction('kt', 'keyboard_type', { text: 'Kay' }, view);
  done(results.pop());
  await driver.runPageAction('pk', 'press_key', { key: 'Enter' }, view);
  done(results.pop());
  for (let i = 0; i < 50 && submitted === null; i++) await new Promise((resolve) => setTimeout(resolve, 100));
  assert.equal(submitted, 'nested:Kay', 'keys with no element reached the focused field two frames deep');
}

/** A form two cross-origin frames deep, two forms from one site, and a frame reloaded at the same address. */
async function nestedAndSiblings(win, driver, results, view) {
  const nested = await readPage(win, driver, view, '/nested');
  assert.equal(
    await submit(driver, results, view, nested, 'nested', 'Grace'),
    'nested:Grace',
    'the nested frame’s form was filled and sent',
  );
  await typeByFocus(win, driver, results, view);
  const siblings = await readPage(win, driver, view, '/siblings');
  assert.equal(
    await submit(driver, results, view, siblings, 'two', 'Linus'),
    'two:Linus',
    'the click landed in the second same-site frame, not the first',
  );
  const again = await readPage(win, driver, view, '/siblings');
  const one = again.find((e) => e.text === 'Create one');
  const frameOne = win.webContents.mainFrame.frames.find((f) => f.url.endsWith('/one'));
  await frameOne.executeJavaScript('location.reload()');
  await new Promise((resolve) => setTimeout(resolve, 800));
  // As in production, the refusal is thrown and commands.ts answers the agent with it.
  await assert.rejects(
    driver.runPageAction('click', 'click', { selector: `[data-ac-id="${one.id}"]` }, view),
    /changed since the page was read/,
    'a frame reloaded at the same address since the read is refused',
  );
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
