/**
 * Unit tests for cdp-front-door.js (and front-door/): the Host and Origin
 * guards, the hidden UI, tabs opened the app's way, admission of every
 * command, and a validation run's limits. Chromium is a loopback fake.
 */
const { describe, it, before, after, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const WebSocket = require('ws');
const { start, localHost, isUi } = require('../../cdp-front-door.js');
const { CDP_SERVER_ERROR, LOCAL_FILES_UNAVAILABLE } = require('../../constants.cjs');
const { NOT_A_WEB_ADDRESS } = require('../../main/tabs/navigation.cjs');

const UI = { id: 'ui', type: 'page', url: 'file:///app/renderer/index.html' };
const PAGE = { id: 'page-1', type: 'page', url: 'https://a.test/' };

/** Targets auto-attach announces, paused for a debugger: another tab's page, and a run tab's iframe. */
const ATTACHED = [
  {
    method: 'Target.attachedToTarget',
    params: {
      sessionId: 's-other',
      targetInfo: { targetId: 'other', type: 'page', url: 'https://b.test/' },
      waitingForDebugger: true,
    },
  },
  {
    method: 'Target.attachedToTarget',
    sessionId: 's-page',
    params: {
      sessionId: 's-frame',
      targetInfo: { targetId: 'frame-1', type: 'iframe', url: 'https://pay.test/' },
      waitingForDebugger: true,
    },
  },
];

/** Every command the fake Chromium received. */
const received = [];

/** A loopback Chromium: /json endpoints and a debugger socket that answers every command. */
async function fakeChromium() {
  const server = http.createServer((req, res) => {
    const port = server.address().port;
    if (req.url === '/json/list') return res.end(JSON.stringify([UI, PAGE]));
    if (req.url === '/json/version') {
      return res.end(JSON.stringify({ webSocketDebuggerUrl: `ws://127.0.0.1:${port}/devtools/browser/b` }));
    }
    res.statusCode = 404;
    res.end('nope');
  });
  const wss = new WebSocket.Server({ server });
  wss.on('connection', (sock) =>
    sock.on('message', (data) => {
      const msg = JSON.parse(data);
      received.push(msg);
      sock.send(JSON.stringify(chromiumReply(msg)));
      if (msg.method === 'Target.setAutoAttach') for (const event of ATTACHED) sock.send(JSON.stringify(event));
    }),
  );
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return { server, wss, port: server.address().port };
}

/** Chromium's answer to one command: its targets for getTargets, an echo otherwise. */
function chromiumReply(msg) {
  if (msg.method === 'Target.getTargets') {
    const infos = [UI, PAGE].map((t) => ({ targetId: t.id, type: t.type, url: t.url }));
    return { id: msg.id, result: { targetInfos: infos } };
  }
  if (msg.method === 'Target.attachToBrowserTarget') return { id: msg.id, result: { sessionId: 's-browser-2' } };
  return { id: msg.id, sessionId: msg.sessionId, result: { echoed: msg.method } };
}

/** An HTTP request to the door; resolves with status and parsed body. */
function request(port, path, { method = 'GET', headers = {} } = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port, path, method, headers }, (res) => {
      let body = '';
      res.on('data', (chunk) => (body += chunk));
      res.on('end', () => resolve({ status: res.statusCode, body: body && JSON.parse(body) }));
    });
    req.on('error', reject);
    req.end();
  });
}

/** Opens a harness socket; resolves once open, with a helper that sends a command and awaits its reply. */
async function harness(url, headers = {}) {
  const sock = new WebSocket(url, { headers });
  await new Promise((resolve, reject) => {
    sock.once('open', resolve);
    sock.once('error', reject);
  });
  const call = (msg) =>
    new Promise((resolve) => {
      const onMessage = (data) => {
        const reply = JSON.parse(data);
        if (reply.id !== msg.id) return;
        sock.off('message', onMessage);
        resolve(reply);
      };
      sock.on('message', onMessage);
      sock.send(JSON.stringify(msg));
    });
  return { sock, call };
}

/** Starts a door over `chromium` with a fake app; resolves once listening. */
async function openDoor(chromium, extra = {}) {
  const app = { tabs: [], admitted: 0, finished: 0, clients: 0, closed: [] };
  const door = start({
    port: 0,
    upstream: chromium.port,
    host: '127.0.0.1',
    tabs: () => app.tabs,
    createTab: (url) => {
      const debuggerApi = { sendCommand: async () => ({ targetInfo: { targetId: 'new-' + url } }) };
      app.tabs.push({
        id: app.tabs.length + 1,
        ready: Promise.resolve(),
        view: { webContents: { debugger: debuggerApi } },
      });
      return app.tabs.length;
    },
    closeTab: (id, options) => app.closed.push([id, options]),
    beginCommand: async () => {
      app.admitted++;
      return () => app.finished++;
    },
    clientChanged: (delta) => (app.clients += delta),
    ...extra,
  });
  await new Promise((resolve) => door.once('listening', resolve));
  return { door, app, port: door.address().port };
}

describe('cdp front door', () => {
  let chromium;
  before(async () => {
    chromium = await fakeChromium();
  });
  after(() => {
    chromium.wss.close();
    chromium.server.close();
  });
  beforeEach(() => mock.method(console, 'log', () => {}));

  it('accepts only IP literals and localhost as the Host', () => {
    assert.equal(localHost({ headers: { host: '[::1]:9222' } }), true);
    assert.equal(localHost({ headers: { host: 'localhost:9222' } }), true);
    assert.equal(localHost({ headers: { host: 'rebind.attacker.test:9222' } }), false);
    assert.equal(localHost({ headers: {} }), false);
  });

  it('recognises the shell and the input shield as UI', () => {
    assert.equal(isUi(UI), true);
    assert.equal(isUi({ type: 'page', url: 'file:///x/renderer/control-shield.html' }), true);
    assert.equal(isUi(PAGE), false);
  });

  it('refuses a request whose Host is not local', async () => {
    const { door, port } = await openDoor(chromium);
    try {
      const res = await request(port, '/json/list', { headers: { host: 'evil.test' } });
      assert.equal(res.status, 403);
    } finally {
      door.close();
    }
  });

  it('lists targets without the browser UI', async () => {
    const { door, port } = await openDoor(chromium);
    try {
      const res = await request(port, '/json/list');
      assert.deepEqual(
        res.body.map((t) => t.id),
        ['page-1'],
      );
    } finally {
      door.close();
    }
  });

  it('points Chromium URLs back at the front door', async () => {
    const { door, port } = await openDoor(chromium);
    try {
      const { body } = await request(port, '/json/version');
      assert.equal(body.webSocketDebuggerUrl, `ws://127.0.0.1:${port}/devtools/browser/b`);
    } finally {
      door.close();
    }
  });

  it('opens a tab only on PUT, through the app and inside an admitted command', async () => {
    const { door, app, port } = await openDoor(chromium);
    try {
      assert.equal((await request(port, '/json/new?https://b.test')).status, 405);
      const res = await request(port, '/json/new?https://b.test', { method: 'PUT' });
      assert.deepEqual([res.status, res.body.id], [200, 'new-https://b.test']);
      assert.deepEqual([app.tabs.length, app.admitted, app.finished], [1, 1, 1]);
    } finally {
      door.close();
    }
  });

  it('proxies other endpoints as admitted commands and passes their status through', async () => {
    const { door, app, port } = await openDoor(chromium);
    try {
      const res = await new Promise((resolve) =>
        http.get({ host: '127.0.0.1', port, path: '/json/other' }, (r) => resolve(r.statusCode)),
      );
      assert.equal(res, 404);
      assert.deepEqual([app.admitted, app.finished], [1, 1]);
    } finally {
      door.close();
    }
  });

  it('answers 502 when Chromium cannot be reached', async () => {
    const { door, port } = await openDoor({ port: 1 });
    try {
      assert.equal((await request(port, '/json/list')).status, 502);
    } finally {
      door.close();
    }
  });

  it('requires the run token and limits a validation run to the listing endpoints', async () => {
    const { door, port } = await openDoor(chromium, { runToken: 'run', allowedTarget: () => true });
    try {
      assert.equal((await request(port, '/json/list')).status, 403);
      assert.equal((await request(port, '/json/new', { method: 'PUT', headers: { 'x-oya-run': 'run' } })).status, 403);
      assert.equal((await request(port, '/json/list', { headers: { 'x-oya-run': 'run' } })).status, 200);
    } finally {
      door.close();
    }
  });

  it('refuses a WebSocket upgrade that carries an Origin or names a hidden target', async () => {
    const { door, port } = await openDoor(chromium);
    try {
      const withOrigin = new WebSocket(`ws://127.0.0.1:${port}/devtools/browser/b`, { origin: 'https://evil.test' });
      await assert.rejects(new Promise((resolve, reject) => withOrigin.on('open', resolve).on('error', reject)));
      const ui = new WebSocket(`ws://127.0.0.1:${port}/devtools/page/ui`);
      await assert.rejects(new Promise((resolve, reject) => ui.on('open', resolve).on('error', reject)));
    } finally {
      door.close();
    }
  });

  it('admits each command, hides the UI from Target.getTargets and counts the client', async () => {
    const { door, app, port } = await openDoor(chromium);
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`);
    try {
      assert.equal(app.clients, 1);
      const reply = await call({ id: 1, method: 'Target.getTargets' });
      assert.deepEqual(
        reply.result.targetInfos.map((t) => t.targetId),
        ['page-1'],
      );
      assert.deepEqual([app.admitted, app.finished], [1, 1]);
    } finally {
      sock.close();
      await new Promise((resolve) => setTimeout(resolve, 20));
      assert.equal(app.clients, 0);
      door.close();
    }
  });

  it('opens and closes tabs through the app on the browser endpoint', async () => {
    const { door, app, port } = await openDoor(chromium);
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`);
    try {
      const created = await call({ id: 1, method: 'Target.createTarget', params: { url: 'https://c.test' } });
      assert.deepEqual(created.result, { targetId: 'new-https://c.test' });
      app.tabs[0].targetId = 'new-https://c.test';
      const closed = await call({ id: 2, method: 'Target.closeTarget', params: { targetId: 'new-https://c.test' } });
      assert.deepEqual(closed.result, { success: true });
      assert.deepEqual(app.closed, [[1, { keepOne: false }]]);
    } finally {
      sock.close();
      door.close();
    }
  });

  it('closes a harness that sends something other than a command', async () => {
    const { door, port } = await openDoor(chromium);
    const { sock } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`);
    try {
      const closed = new Promise((resolve) => sock.once('close', resolve));
      sock.send('not json');
      await closed;
    } finally {
      door.close();
    }
  });

  it('refuses a command that could not be admitted, with a CDP error', async () => {
    const { door, port } = await openDoor(chromium, {
      beginCommand: async () => {
        throw new Error('Automation paused for human control');
      },
    });
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`);
    try {
      const reply = await call({ id: 1, method: 'Browser.getVersion' });
      assert.deepEqual(reply.error, { code: CDP_SERVER_ERROR, message: 'Automation paused for human control' });
    } finally {
      sock.close();
      door.close();
    }
  });

  it('refuses a file: url on /json/new with 400, and opens no tab', async () => {
    const { door, app, port } = await openDoor(chromium);
    try {
      const res = await request(port, '/json/new?file:///etc/hosts', { method: 'PUT' });
      assert.deepEqual([res.status, res.body.error, app.tabs.length], [400, NOT_A_WEB_ADDRESS, 0]);
    } finally {
      door.close();
    }
  });

  it('answers a file: Page.navigate on the page endpoint with a CDP error and never forwards it', async () => {
    const { door, port } = await openDoor(chromium);
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/page/page-1`);
    try {
      received.length = 0;
      const reply = await call({ id: 7, method: 'Page.navigate', params: { url: 'file:///etc/hosts' } });
      assert.deepEqual(reply.error, { code: CDP_SERVER_ERROR, message: NOT_A_WEB_ADDRESS });
      assert.equal(received.filter((m) => m.method === 'Page.navigate').length, 0);
      const ok = await call({ id: 8, method: 'Page.navigate', params: { url: 'https://a.test/' } });
      assert.equal(ok.result.echoed, 'Page.navigate');
    } finally {
      sock.close();
      door.close();
    }
  });

  it('refuses a relay an upload with a CDP error, and stubs its download setting without forwarding', async () => {
    const { door, port } = await openDoor(chromium, { relayToken: 'relay' });
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`, { 'x-oya-relay': 'relay' });
    try {
      received.length = 0;
      const upload = await call({
        id: 3,
        sessionId: 's-page',
        method: 'DOM.setFileInputFiles',
        params: { files: ['/etc/passwd'] },
      });
      assert.deepEqual([upload.sessionId, upload.error.message], ['s-page', LOCAL_FILES_UNAVAILABLE]);
      const stub = await call({
        id: 4,
        method: 'Browser.setDownloadBehavior',
        params: { behavior: 'allow', downloadPath: '/tmp' },
      });
      assert.deepEqual(stub.result, {});
      assert.deepEqual(
        received.map((m) => m.method),
        [],
      );
    } finally {
      sock.close();
      door.close();
    }
  });

  it('gives a second browser session the browser endpoint\u2019s handling: hidden UI and tabs the app\u2019s way', async () => {
    const { door, app, port } = await openDoor(chromium);
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`);
    try {
      const { result } = await call({ id: 1, method: 'Target.attachToBrowserTarget' });
      const sessionId = result.sessionId;
      const listed = await call({ id: 2, sessionId, method: 'Target.getTargets' });
      assert.deepEqual(
        listed.result.targetInfos.map((t) => t.targetId),
        ['page-1'],
      );
      const opened = await call({ id: 3, sessionId, method: 'Target.createTarget', params: { url: 'https://c.test' } });
      assert.deepEqual(
        [opened.sessionId, opened.result.targetId, app.tabs.length],
        [sessionId, 'new-https://c.test', 1],
      );
    } finally {
      sock.close();
      door.close();
    }
  });

  it('refuses a harness a new tab that could not be protected, with a CDP error', async () => {
    const failedTab = (url) => {
      const debuggerApi = { sendCommand: async () => ({ targetInfo: { targetId: 'new-' + url } }) };
      const tab = {
        id: 1,
        protection: 'failed',
        setup: Promise.resolve(),
        view: { webContents: { debugger: debuggerApi } },
      };
      app.tabs.push(tab);
      return tab.id;
    };
    const { door, app, port } = await openDoor(chromium, { createTab: (url) => failedTab(url) });
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`);
    try {
      const reply = await call({ id: 9, method: 'Target.createTarget', params: { url: 'https://d.test' } });
      assert.match(reply.error.message, /could not be protected/);
      assert.equal(reply.result, undefined);
      assert.deepEqual(app.closed, [[1, { keepOne: false }]], 'the refused tab was left open for a harness to find');
    } finally {
      sock.close();
      door.close();
    }
  });

  it('lets a relay through without local admission or a client count', async () => {
    const { door, app, port } = await openDoor(chromium, { relayToken: 'relay' });
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`, { 'x-oya-relay': 'relay' });
    try {
      await call({ id: 1, method: 'Browser.getVersion' });
      assert.deepEqual([app.admitted, app.clients], [0, 0]);
    } finally {
      sock.close();
      door.close();
    }
  });

  it('resumes and lets go of a hidden target, and lets a run tab’s iframe into the run', async () => {
    const { door, port } = await openDoor(chromium, { runToken: 'run', allowedTarget: (id) => id === 'page-1' });
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`, { 'x-oya-run': 'run' });
    const seen = [];
    sock.on('message', (data) => seen.push(JSON.parse(data)));
    try {
      received.length = 0;
      await call({ id: 1, method: 'Target.setAutoAttach', params: { autoAttach: true, waitForDebuggerOnStart: true } });
      await new Promise((resolve) => setTimeout(resolve, 50));
      const attached = seen
        .filter((m) => m.method === 'Target.attachedToTarget')
        .map((m) => m.params.targetInfo.targetId);
      assert.deepEqual(attached, ['frame-1']);
      const released = received.filter((m) => m.id < 0).map((m) => [m.method, m.sessionId || m.params?.sessionId]);
      assert.deepEqual(released, [
        ['Runtime.runIfWaitingForDebugger', 's-other'],
        ['Target.detachFromTarget', 's-other'],
      ]);
      assert.ok(!seen.some((m) => m.id < 0), 'the bridge’s own replies never reach the harness');
      const frame = await call({ id: 2, method: 'Target.getTargetInfo', params: { targetId: 'frame-1' } });
      assert.equal(frame.result.echoed, 'Target.getTargetInfo');
    } finally {
      sock.close();
      door.close();
    }
  });

  it('stubs downloads and refuses other browser commands in a validation run', async () => {
    const { door, port } = await openDoor(chromium, { runToken: 'run', allowedTarget: (id) => id === 'page-1' });
    const { sock, call } = await harness(`ws://127.0.0.1:${port}/devtools/browser/b`, { 'x-oya-run': 'run' });
    try {
      assert.deepEqual((await call({ id: 1, method: 'Browser.setDownloadBehavior' })).result, {});
      assert.equal(
        (await call({ id: 2, method: 'Browser.close' })).error.message,
        'Browser command unavailable for validation',
      );
      const outside = await call({ id: 3, method: 'Target.attachToTarget', params: { targetId: 'other' } });
      assert.equal(outside.error.message, 'Target is outside this validation');
      assert.equal((await call({ id: 4, method: 'Browser.getVersion' })).result.echoed, 'Browser.getVersion');
    } finally {
      sock.close();
      door.close();
    }
  });
});
