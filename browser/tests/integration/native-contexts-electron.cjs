/** Real native contexts, cookies, local storage and downloads through Oya's external compatibility boundary. */
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { once } = require('node:events');
const { randomBytes } = require('node:crypto');
const electron = require('electron');
const { WebSocket } = require('ws');
const { AppNativeBackend } = require('../../src/main/app/native-cdp.ts');
const { startNativeFrontDoor } = require('../../src/main/native-front-door/index.ts');
const { WindowTabEvents } = require('../../src/main/windows/tab-events.ts');
const { app, BrowserWindow } = electron;
const root = fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()), 'oya-native-contexts-'));
app.setPath('userData', path.join(root, 'profile'));
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const timeout = setTimeout(() => {
  console.error('Native context test timed out');
  app.exit(1);
}, 60000);
/** Each tab uses a real native session and renderer; no inspector access is permitted. */
function dependencies() {
  const tabs = [],
    windows = new Map(),
    tabEvents = new WindowTabEvents();
  let next = 0;
  const deps = {
    electron,
    nativeBrowsing: true,
    persona: { active: null },
    governance: { configuration: null, allowed: () => true, install() {} },
    control: { busy: false, localHeld: false, connected: false, state: { mode: 'offline' } },
    actions: { waitForTabReady: (tab) => tab.ready },
    tabs: {
      createTab(url, activate, _options, session) {
        assert.ok(session, 'fixture only creates explicitly private tabs');
        assert.equal(activate, true, 'private targets must mount before agent readiness and input');
        const win = new BrowserWindow({
          show: false,
          webPreferences: { session, sandbox: true, contextIsolation: true, nodeIntegration: false },
        });
        Object.defineProperty(win.webContents, 'debugger', {
          get() {
            throw Error('Internal CDP forbidden');
          },
        });
        const tab = { id: ++next, home: false, protection: 'protected', view: { webContents: win.webContents } };
        tabs.push(tab);
        windows.set(tab.id, win);
        tab.ready = win.loadURL(url);
        tabEvents.changed();
        return tab.id;
      },
      closeTab(id) {
        const i = tabs.findIndex((t) => t.id === id);
        if (i < 0) return;
        tabs.splice(i, 1);
        windows.get(id).destroy();
        windows.delete(id);
        tabEvents.changed();
      },
    },
    windows: { allTabs: () => tabs, tabEvents, owner: () => deps },
  };
  return deps;
}
/** Authenticated external client owns only its own context and flat-session namespace. */
async function client(url, token) {
  const socket = new WebSocket(url, { headers: { authorization: `Bearer ${token}` } });
  await once(socket, 'open');
  let id = 0;
  const pending = new Map(),
    events = [];
  socket.on('message', (bytes) => {
    const r = JSON.parse(bytes);
    if (!('id' in r)) return events.push(r);
    const p = pending.get(r.id);
    pending.delete(r.id);
    r.error ? p.reject(Error(r.error.message)) : p.resolve(r.result);
  });
  return {
    socket,
    events,
    call(method, params = {}, sessionId) {
      return new Promise((resolve, reject) => {
        const seq = ++id;
        pending.set(seq, { resolve, reject });
        socket.send(JSON.stringify({ id: seq, method, params, sessionId }));
      });
    },
  };
}
/** Wait for native download events rather than re-reading page content. */
async function until(read) {
  for (let i = 0; i < 500; i++) {
    const result = read();
    if (result) return result;
    await new Promise((r) => setTimeout(r, 10));
  }
  throw Error('Native event missing');
}
/** Deterministic response bytes prove the actual response, not a synthetic or re-fetched file. */
const payload = Buffer.from('Oya native download 日本語\n'.repeat(1000));
/** Exercise browser-context and download commands through authenticated external sessions. */
async function run() {
  await app.whenReady();
  await require('./native-context-readiness.cjs')(electron);
  const hits = { bytes: 0, large: 0, paused: 0, blocked: 0 };
  const server = http.createServer((req, res) => {
    if (req.url.startsWith('/filter/')) {
      hits[req.url] = (hits[req.url] || 0) + 1;
      res.setHeader('Cache-Control', 'no-store');
      if (req.url === '/filter/pixel') {
        res.setHeader('Content-Type', 'image/svg+xml');
        return res.end('<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>');
      }
      return res.end(req.url);
    }
    if (req.url === '/cookie-echo') return res.end(req.headers.cookie || '');
    if (req.url === '/cookie-server') {
      res.setHeader('Set-Cookie', 'server_priority=actual; Priority=High; HttpOnly; SameSite=Strict; Path=/');
      return res.end('set');
    }
    if (req.url === '/bytes') {
      hits.bytes++;
      res.end(Buffer.from([0, 255, 128, 17, 0, 65, 66, 67]));
      return;
    }
    if (req.url === '/large') {
      hits.large++;
      res.end(Buffer.alloc(1024 * 1024, 65));
      return;
    }
    if (req.url === '/paused') {
      hits.paused++;
      res.end('continued');
      return;
    }
    if (req.url === '/blocked') {
      hits.blocked++;
      res.end('should never arrive');
      return;
    }
    if (['/child-continued', '/child-cancelled', '/child-sibling'].includes(req.url)) {
      const key = req.url.slice(1);
      hits[key] = (hits[key] || 0) + 1;
      res.end(key);
      return;
    }
    if (req.url.startsWith('/child')) {
      res.setHeader('Content-Type', 'text/html');
      res.end('<!doctype html><title>Native child</title><input>');
      return;
    }
    if (req.url.startsWith('/download')) {
      res.writeHead(200, {
        'Content-Type': 'application/octet-stream',
        'Content-Disposition': 'attachment; filename="native.txt"',
        'Content-Length': payload.length,
      });
      res.end(payload);
      return;
    }
    if (req.url === '/slow') {
      res.writeHead(200, {
        'Content-Type': 'application/octet-stream',
        'Content-Disposition': 'attachment; filename="slow.txt"',
      });
      const interval = setInterval(() => res.write(Buffer.alloc(1024)), 20);
      res.on('close', () => clearInterval(interval));
      return;
    }
    res.setHeader('Content-Type', 'text/html');
    res.end(
      `<title>Native contexts</title><a id="download" href="/download">Download</a><a id="slow" href="/slow">Slow</a><iframe src="http://localhost:${server.address().port}/child"></iframe><iframe src="http://localhost:${server.address().port}/child"></iframe>`,
    );
  });
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const deps = dependencies(),
    token = randomBytes(32).toString('hex');
  const door = startNativeFrontDoor({
    port: 0,
    token,
    backend: new AppNativeBackend(deps),
    beginCommand: () => () => {},
    clientChanged() {},
  });
  await once(door, 'listening');
  const endpoint = `ws://127.0.0.1:${door.address().port}/devtools/browser`;
  const a = await client(endpoint, token),
    b = await client(endpoint, token);
  try {
    await assert.rejects(a.call('Target.createBrowserContext', {}), /disposeOnDetach/);
    const context = (await a.call('Target.createBrowserContext', { disposeOnDetach: true })).browserContextId;
    const second = (await a.call('Target.createBrowserContext', { disposeOnDetach: true })).browserContextId;
    assert.deepEqual((await b.call('Target.getBrowserContexts')).browserContextIds, []);
    const url = `http://127.0.0.1:${server.address().port}`;
    const target = (await a.call('Target.createTarget', { url, browserContextId: context })).targetId;
    const other = (await a.call('Target.createTarget', { url, browserContextId: second })).targetId;
    const session = (await a.call('Target.attachToTarget', { targetId: target, flatten: true })).sessionId;
    const session2 = (await a.call('Target.attachToTarget', { targetId: other, flatten: true })).sessionId;
    assert.equal((await b.call('Target.getTargets')).targetInfos.length, 0);
    await assert.rejects(b.call('Target.createTarget', { url, browserContextId: context }), /foreign/);
    await assert.rejects(b.call('Target.attachToTarget', { targetId: target, flatten: true }), /outside/);
    await require('./native-cookie-checks.cjs')(a, b, context, second, session, session2, url, deps.control);
    const evaluate = (expression, s = session) => a.call('Runtime.evaluate', { expression, returnByValue: true }, s);
    await evaluate('document.cookie="private=one";localStorage.setItem("private","one")');
    assert.deepEqual((await evaluate('[document.cookie,localStorage.getItem("private")]', session2)).result.value, [
      '',
      null,
    ]);
    await a.call('Browser.setDownloadBehavior', {
      browserContextId: context,
      behavior: 'allowAndName',
      downloadPath: root,
      eventsEnabled: true,
    });
    await evaluate('document.querySelector("#download").click()');
    const begun = await until(() => a.events.find((e) => e.method === 'Browser.downloadWillBegin'));
    const done = await until(() =>
      a.events.find(
        (e) =>
          e.method === 'Browser.downloadProgress' &&
          e.params.guid === begun.params.guid &&
          e.params.state === 'completed',
      ),
    );
    assert.equal(done.params.filePath, path.join(root, begun.params.guid));
    assert.deepEqual(fs.readFileSync(done.params.filePath), payload);
    await evaluate('document.querySelector("#slow").click()');
    const slow = await until(() =>
      a.events.find((e) => e.method === 'Browser.downloadWillBegin' && e.params.guid !== begun.params.guid),
    );
    await assert.rejects(
      b.call('Browser.cancelDownload', { browserContextId: context, guid: slow.params.guid }),
      /foreign/,
    );
    await assert.rejects(
      a.call('Browser.cancelDownload', { browserContextId: second, guid: slow.params.guid }),
      /foreign/,
    );
    await a.call('Browser.cancelDownload', { browserContextId: context, guid: slow.params.guid });
    await until(() =>
      a.events.find(
        (e) =>
          e.method === 'Browser.downloadProgress' &&
          e.params.guid === slow.params.guid &&
          e.params.state === 'canceled',
      ),
    );
    await require('./native-network-checks.cjs')(a, b, session, session2, deps, url, hits);
    const networkFrame = deps.windows.allTabs()[0].view.webContents.mainFrame;
    assert.ok(
      networkFrame.frames.every((frame) => frame.processId !== networkFrame.processId),
      'network children must use separate renderer processes',
    );
    await require('./native-network-frames-checks.cjs')(a, session, hits);
    await require('./native-network-pattern-checks.cjs')(a, session, hits);
    await require('./native-network-errors-checks.cjs')(a, session, hits);
    await a.call('Target.disposeBrowserContext', { browserContextId: context });
    await assert.rejects(evaluate('1'), /closed|outside|unavailable/);
    a.socket.terminate();
    await until(() => deps.windows.allTabs().length === 0);
    console.log(
      'PASS: native ephemeral contexts, cookie/storage isolation, cross-connection privacy, actual download bytes/GUID path, scoped cancellation and disconnect disposal; debugger forbidden',
    );
  } finally {
    a.socket.terminate();
    b.socket.terminate();
    await new Promise((r) => door.close(r));
    for (const tab of [...deps.windows.allTabs()]) deps.tabs.closeTab(tab.id);
    server.closeAllConnections();
    await new Promise((r) => server.close(r));
  }
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
