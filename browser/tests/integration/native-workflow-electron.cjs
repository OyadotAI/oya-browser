/** Real native workflow validation with run-owned tabs, trusted input and a fatal debugger boundary. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const { once } = require('node:events');
const { app, BrowserWindow } = require('electron');
const { validateNative } = require('../../src/main/workflow/native-validation.ts');
const { normalizeDraft } = require('../../src/workflow/index.ts');
const { PageDriver } = require('../../src/main/actions/driver.ts');
const { World } = require('../../src/main/native/index.ts');
const { Mouse } = require('../../src/main/input/mouse.ts');
const { Keyboard } = require('../../src/main/input/keyboard.ts');
if (!process.env.OYA_NATIVE_WORKFLOW_PROFILE) throw Error('Run native-workflow.mjs for parent-owned profile cleanup');
app.setPath('userData', process.env.OYA_NATIVE_WORKFLOW_PROFILE);
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Native workflow timed out');
  app.exit(1);
}, 60000);
const windows = [],
  tabs = [],
  leftOpen = new Set();
let sequence = 0,
  human = false,
  clients = 0,
  inFlight = 0;
const control = {
  snapshot: () => ({ mode: human ? 'human' : 'agent', mine: human }),
  change: async () => {
    throw Error('Workflow cannot automatically seize human control');
  },
  localClient: (delta) => {
    clients += delta;
  },
  beginLocalCommand: async () => {
    if (human) throw Error('Automation paused for human control');
    inFlight++;
    return () => {
      inFlight--;
    };
  },
};
/** Exact native windows stand in for the shell's tab registry, with actual protection/load promises. */
function createTab(url) {
  const win = new BrowserWindow({
    show: true,
    width: 800,
    height: 600,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInSubFrames: true,
      preload: path.resolve(__dirname, '../../out/preload/recording.js'),
    },
  });
  windows.push(win);
  Object.defineProperty(win.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  const tab = { id: ++sequence, title: '', url, protection: 'protected', view: { webContents: win.webContents } };
  tab.ready = win.loadURL(url);
  tabs.push(tab);
  return tab.id;
}
/** Close only ids the native run retained, leaving unrelated browsing intact. */
function closeTab(id) {
  const index = tabs.findIndex((tab) => tab.id === id);
  if (index < 0) return;
  const [tab] = tabs.splice(index, 1);
  windows.find((win) => !win.isDestroyed() && win.webContents === tab.view.webContents)?.destroy();
}
const world = new World({
  analyzerScript: fs.readFileSync(path.join(__dirname, '../../scripts/analyzer.js'), 'utf8'),
  worldName: 'workflow-native-test',
});
const driver = new PageDriver({
  mouse: new Mouse(),
  keyboard: new Keyboard(process.platform),
  getActiveView: () => {
    throw Error('Workflow must never resolve the application active tab');
  },
  tabs: () => tabs,
  activeTabId: () => null,
  injectScripts: (view) => world.ensure(view),
  worldEval: (view, code) =>
    world.evaluate(view, code).catch((error) => {
      console.error('Native workflow evaluation failed:', error);
      throw error;
    }),
  pullCookiesFor: async () => {},
  sendResult: () => {
    throw Error('Workflow must own its response sink');
  },
});
/** Retain fixture-only pointer diagnostics for cross-process input failures. */
const pointerTrace = [];
const nativeMove = driver.mouse.move.bind(driver.mouse);
driver.mouse.move = async (view, x, y) => {
  pointerTrace.push({ x, y, url: view.webContents.getURL() });
  return nativeMove(view, x, y);
};
/** Observe completion through the workspace's public event contract. */
async function start(steps, event = () => {}) {
  let complete;
  const done = new Promise((resolve) => {
    complete = resolve;
  });
  const messages = [];
  const session = await validateNative({
    draft: normalizeDraft({ steps }),
    options: {},
    driver,
    control,
    tabs: () => tabs,
    createTab,
    closeTab,
    leftOpen,
    event: (message) => {
      messages.push(message);
      event(message);
      if (message.type === 'finished') complete(message);
    },
  });
  return { session, done, messages };
}
/** Complete actual form effects and verify human ownership before the next native dispatch. */
async function run() {
  await app.whenReady();
  const server = http.createServer((req, res) => {
    if (req.url.startsWith('/parity') || req.url.startsWith('/outer') || req.url.startsWith('/child')) {
      res.setHeader('Content-Type', 'text/html');
      return res.end(require('./native-workflow-parity.cjs').page(req.url, server.address().port));
    }
    res.setHeader('Content-Type', 'text/html');
    res.end(
      '<!doctype html><input id=name value=old><button id=save onclick="window.saved=document.querySelector(\'input\').value;window.clicked=event.isTrusted">Save</button><script>window.trusted=[];document.querySelector("input").addEventListener("input",e=>trusted.push(e.isTrusted))</script>',
    );
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const url = `http://127.0.0.1:${server.address().port}/`;
  const step = (id, action, rest = {}) => ({ id, action, tab: 'main', timeout: 3000, ...rest });
  const css = (value) => [{ kind: 'css', value }];
  try {
    const before = sequence;
    await assert.rejects(
      start([step('unsupported', 'upload_file', { candidates: css('#upload'), file: 'relative-path.txt' })]),
      /absolute file path/,
    );
    assert.equal(sequence, before);
    assert.equal(clients, 0);
    const successful = await start([
      step('navigate', 'navigate', { url }),
      step('empty', 'type', { candidates: css('#name'), text: '' }),
      step('empty-value', 'assert_value', { candidates: css('#name'), expected: '' }),
      step('type', 'type', { candidates: css('#name'), text: 'native 日本語' }),
      step('value', 'assert_value', { candidates: css('#name'), expected: 'native 日本語' }),
      step('click', 'click', { candidates: css('#save') }),
    ]);
    const result = await successful.done;
    assert.equal(result.status, 'succeeded', JSON.stringify(successful.messages));
    const wc = tabs.at(-1).view.webContents;
    assert.deepEqual(await wc.executeJavaScript('[saved,clicked,trusted.length>0&&trusted.every(Boolean)]'), [
      'native 日本語',
      true,
      true,
    ]);
    const halted = await start(
      [step('navigate', 'navigate', { url }), step('click', 'click', { candidates: css('#save') })],
      (message) => {
        if (message.type === 'event' && message.event.stepId === 'navigate' && message.event.status === 'passed')
          human = true;
      },
    );
    assert.equal((await halted.done).status, 'failed');
    assert.equal(await tabs.at(-1).view.webContents.executeJavaScript('typeof clicked'), 'undefined');
    assert.equal(clients, 0);
    assert.equal(inFlight, 0);
    human = false;
    await require('./native-workflow-parity.cjs').check({
      start,
      step,
      css,
      url,
      tabs,
      profile: process.env.OYA_NATIVE_WORKFLOW_PROFILE,
      pointerTrace,
    });
    console.log(
      'PASS native workflow: real protected navigation, trusted Unicode field replacement/click, value assertions, unsupported preflight without side effects, human takeover and balanced control; debugger forbidden',
    );
  } finally {
    for (const win of windows) if (!win.isDestroyed()) win.destroy();
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
}
run()
  .then(() => {
    clearTimeout(deadline);
    app.exit(0);
  })
  .catch((error) => {
    console.error(error);
    app.exit(1);
  });
