/** Exercise the authenticated external compatibility protocol on real Oya pages with debugger access forbidden. */
const assert = require('node:assert/strict');
const { once } = require('node:events');
const http = require('node:http');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { randomBytes } = require('node:crypto');
const { WebSocket } = require('ws');
const { app, BrowserWindow } = require('electron');
const { WindowTabEvents } = require('../../src/main/windows/tab-events.ts');
const { AppNativeBackend } = require('../../src/main/app/native-cdp.ts');
const { startNativeFrontDoor } = require('../../src/main/native-front-door/index.ts');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-native-front-door-'));
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const timeout = setTimeout(() => {
  console.error('Native front-door test timed out');
  app.exit(1);
}, 60000);
/** Disposable localhost fixture, never an external website or a signed-in profile. */
async function run() {
  await app.whenReady();
  const pages = http.createServer((req, res) => {
    res.setHeader('Content-Type', 'text/html');
    if (req.url.startsWith('/child')) return res.end('<!doctype html><title>Child</title><input id=child>');
    res.end(
      '<!doctype html><title>Native fixture</title><input id="search" value="initial"><button id="log" onclick="console.warn(\'native-console-check\')">Log</button><iframe src="http://localhost:PORT/child"></iframe><iframe src="http://localhost:PORT/child"></iframe>'.replaceAll(
        'PORT',
        String(pages.address().port),
      ),
    );
  });
  pages.listen(0, '127.0.0.1');
  await once(pages, 'listening');
  const win = new BrowserWindow({
    show: false,
    webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  const wc = win.webContents;
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  await wc.loadURL(`http://127.0.0.1:${pages.address().port}/`);
  const tab = { id: 1, home: false, protection: 'protected', view: { webContents: wc } };
  const control = { busy: false, localHeld: false, connected: false, state: { mode: 'offline' } };
  const tabEvents = new WindowTabEvents();
  const backend = new AppNativeBackend({
    windows: { allTabs: () => [tab], tabEvents, owner: () => ({ shell: { window: win } }) },
    tabs: { activateTab: (id) => assert.equal(id, tab.id) },
    control,
    governance: { allowed: (url) => !url.endsWith('/blocked') },
  });
  const token = randomBytes(32).toString('hex');
  const door = startNativeFrontDoor({ port: 0, token, backend, beginCommand: () => () => {}, clientChanged() {} });
  await once(door, 'listening');
  const target = backend.targets()[0].targetId;
  const url = `ws://127.0.0.1:${door.address().port}/devtools/page/${target}`;
  const a = await client(url, token),
    b = await client(url, token);
  try {
    const capabilities = await a.call('Oya.getCapabilities');
    assert.equal(capabilities.compatibility, 'partial');
    assert.ok(capabilities.methods.some((m) => m.method === 'Oya.navigateToHistoryEntry'));
    assert.ok(!capabilities.methods.some((m) => m.method === 'Fetch.fulfillRequest'));
    await inspect(a, b, wc, control);
    await pageOperations(a, b, wc, control);
    await require('./native-runtime-checks.cjs')(a, b, wc, control);
    await require('./native-runtime-properties-checks.cjs')(a);
    await require('./native-runtime-frames-checks.cjs')(a, b, wc);
    await discovery(`ws://127.0.0.1:${door.address().port}/devtools/browser`, token, target, tabEvents, wc, control);
    await require('./native-history-checks.cjs')(a, b, wc);
    await a.call('Page.bringToFront');
    assert.equal(win.isVisible(), true);
  } finally {
    a.socket.terminate();
    b.socket.terminate();
    await new Promise((resolve) => door.close(resolve));
    win.destroy();
    pages.closeAllConnections();
    await new Promise((resolve) => pages.close(resolve));
  }
  console.log(
    'PASS: real Oya native CDP DOM, frame trees, viewport metrics, stale/foreign nodes, native logs, page readiness/reload, focus and trusted Unicode insertion, target discovery/activation, human ownership, cleanup; no debugger backend',
  );
}
/** WebSocket-only client; it has no direct access to the native backend. */
async function client(url, token) {
  const socket = new WebSocket(url, { headers: { authorization: `Bearer ${token}` } });
  await once(socket, 'open');
  let sequence = 0;
  const pending = new Map(),
    events = [];
  socket.on('message', (raw) => {
    const message = JSON.parse(raw.toString());
    if (!('id' in message)) return events.push(message);
    const task = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) task.reject(Error(message.error.message));
    else task.resolve(message.result);
  });
  return {
    socket,
    events,
    call(method, params = {}, sessionId) {
      const id = ++sequence;
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        socket.send(JSON.stringify({ id, method, params, sessionId }));
      });
    },
  };
}
/** Assertions cover actual native renderer results, not command acceptance alone. */
async function inspect(a, b, wc, control) {
  await a.call('Emulation.setDeviceMetricsOverride', { width: 480, height: 640, deviceScaleFactor: 2, mobile: false });
  assert.equal(await wc.executeJavaScript('innerWidth'), 480);
  assert.equal(await wc.executeJavaScript('devicePixelRatio'), 2);
  await assert.rejects(b.call('Emulation.clearDeviceMetricsOverride'), /another connection/);
  await a.call('Emulation.clearDeviceMetricsOverride');
  assert.notEqual(await wc.executeJavaScript('innerWidth'), 480);
  const firstTree = (await a.call('Page.getFrameTree')).frameTree;
  assert.equal(firstTree.childFrames.length, 2);
  assert.equal(firstTree.childFrames[0].frame.url, firstTree.childFrames[1].frame.url);
  assert.notEqual(firstTree.childFrames[0].frame.id, firstTree.childFrames[1].frame.id);
  assert.equal(firstTree.childFrames[0].frame.parentId, firstTree.frame.id);
  assert.equal((await a.call('Page.getFrameTree')).frameTree.frame.loaderId, firstTree.frame.loaderId);
  const { root } = await a.call('DOM.getDocument', { depth: 2 });
  assert.equal(root.nodeName, '#document');
  const { nodeId } = await a.call('DOM.querySelector', { nodeId: root.nodeId, selector: '#search' });
  assert.ok(nodeId > 0);
  assert.deepEqual((await a.call('DOM.getAttributes', { nodeId })).attributes, ['id', 'search', 'value', 'initial']);
  assert.match((await a.call('DOM.getOuterHTML', { nodeId })).outerHTML, /value="initial"/);
  await b.call('DOM.getDocument');
  await assert.rejects(b.call('DOM.getAttributes', { nodeId }), /foreign/);
  await assert.rejects(a.call('DOM.getDocument', { pierce: true }), /unsupported/);
  await assert.rejects(a.call('Runtime.evaluate', { expression: '1+1', throwOnSideEffect: true }), /Unsupported/);
  assert.equal(await wc.executeJavaScript('Object.keys(window).some(k=>k.startsWith("oya-inspect-"))'), false);
  await a.call('Log.enable');
  await wc.executeJavaScript('console.warn("native-console-check")');
  await a.call('DOM.getAttributes', { nodeId });
  assert.ok(a.events.some((e) => e.method === 'Log.entryAdded' && e.params.entry.text === 'native-console-check'));
  control.localHeld = true;
  await wc.executeJavaScript('console.warn("human-private")');
  control.localHeld = false;
  await a.call('DOM.getAttributes', { nodeId });
  assert.ok(!a.events.some((e) => e.params.entry.text === 'human-private'));
  await a.call('Log.disable');
  const count = a.events.length;
  await wc.executeJavaScript('console.warn("disabled")');
  await a.call('DOM.getAttributes', { nodeId });
  assert.equal(a.events.length, count);
  await wc.loadURL(wc.getURL());
  await assert.rejects(a.call('DOM.getAttributes', { nodeId }), /unavailable/);
  assert.notEqual((await a.call('Page.getFrameTree')).frameTree.frame.loaderId, firstTree.frame.loaderId);
  const newer = await a.call('DOM.getDocument');
  assert.notEqual(newer.root.nodeId, root.nodeId);
  await assert.rejects(a.call('DOM.getAttributes', { nodeId }), /foreign/);
}
run().then(
  () => {
    clearTimeout(timeout);
    app.exit(0);
  },
  (error) => {
    console.error(error);
    clearTimeout(timeout);
    app.exit(1);
  },
);

/** Real native tab changes reach only authenticated discovery subscribers, never human-held activity. */
async function discovery(url, token, target, tabEvents, wc, control) {
  const browser = await client(url, token);
  try {
    await browser.call('Target.setDiscoverTargets', { discover: true });
    assert.equal(browser.events[0].method, 'Target.targetCreated');
    assert.equal(browser.events[0].params.targetInfo.targetId, target);
    await wc.executeJavaScript('document.title="Native discovery title"');
    tabEvents.changed();
    await browser.call('Browser.getVersion');
    assert.ok(
      browser.events.some(
        (e) => e.method === 'Target.targetInfoChanged' && e.params.targetInfo.title === 'Native discovery title',
      ),
    );
    const count = browser.events.length;
    control.localHeld = true;
    await wc.executeJavaScript('document.title="Private human title"');
    tabEvents.changed();
    await browser.call('Browser.getVersion');
    assert.equal(browser.events.length, count);
    control.localHeld = false;
    await browser.call('Target.setDiscoverTargets', { discover: false });
    tabEvents.changed();
    await browser.call('Browser.getVersion');
    assert.equal(browser.events.length, count);
    await browser.call('Target.activateTarget', { targetId: target });
    await automatic(browser, target);
  } finally {
    browser.socket.terminate();
  }
}
/** A standard flat automatic session reaches real native DOM and is revoked on disable. */
async function automatic(browser, target) {
  const params = { autoAttach: true, flatten: true, waitForDebuggerOnStart: false, filter: [{ type: 'page' }] };
  await browser.call('Target.setAutoAttach', params);
  const attached = browser.events.find((e) => e.method === 'Target.attachedToTarget');
  assert.equal(attached.params.targetInfo.targetId, target);
  assert.equal(attached.params.waitingForDebugger, false);
  const session = attached.params.sessionId;
  const doc = await browser.call('DOM.getDocument', {}, session);
  assert.equal(doc.root.nodeName, '#document');
  await browser.call('Target.setAutoAttach', { ...params, autoAttach: false });
  assert.ok(browser.events.some((e) => e.method === 'Target.detachedFromTarget' && e.params.sessionId === session));
  await assert.rejects(browser.call('DOM.getDocument', {}, session), /outside/);
}
/** Real DOM focus, composed text and lifecycle delivery use native APIs with debugger access forbidden. */
async function pageOperations(a, b, wc, control) {
  const { root } = await a.call('DOM.getDocument');
  const { nodeId } = await a.call('DOM.querySelector', { nodeId: root.nodeId, selector: '#search' });
  await a.call('DOM.focus', { nodeId });
  wc.selectAll();
  await wc.executeJavaScript(
    'document.querySelector("input").addEventListener("input", e => window.trustedInput = e.isTrusted)',
  );
  await a.call('Input.insertText', { text: 'Native 日本語 🙂' });
  assert.equal(await wc.executeJavaScript('document.querySelector("input").value'), 'Native 日本語 🙂');
  assert.equal(await wc.executeJavaScript('window.trustedInput'), true);
  const firstInsert = wc._insertTextOya('!');
  await assert.rejects(wc._insertTextOya('must-not-queue'), /already pending/);
  await firstInsert;
  assert.equal(await wc.executeJavaScript('document.querySelector("input").value'), 'Native 日本語 🙂!');

  await assert.rejects(b.call('DOM.focus', { nodeId }), /foreign|unavailable/);
  await a.call('DOM.scrollIntoViewIfNeeded', { nodeId });
  await a.call('Page.enable');
  await a.call('Page.enable');
  await b.call('Page.enable');
  const before = a.events.length;
  const loaded = once(wc, 'did-finish-load');
  await a.call('Page.reload', { ignoreCache: true });
  await loaded;
  await a.call('Page.getFrameTree');
  const delivered = a.events.slice(before);
  assert.equal(delivered.filter((e) => e.method === 'Page.domContentEventFired').length, 1);
  assert.equal(delivered.filter((e) => e.method === 'Page.loadEventFired').length, 1);
  await assert.rejects(a.call('DOM.focus', { nodeId }), /unavailable|foreign/);
  await a.call('Page.disable');
  const stopped = a.events.length;
  control.localHeld = true;
  await wc.loadURL(wc.getURL());
  control.localHeld = false;
  await b.call('Page.getFrameTree');
  assert.equal(a.events.length, stopped);
  assert.equal(b.events.filter((e) => e.method === 'Page.loadEventFired').length, 1);
  await b.call('Page.disable');
  await a.call('Page.stopLoading');
}
