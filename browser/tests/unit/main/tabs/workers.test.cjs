/**
 * Unit tests for main/tabs/workers.cjs: service and shared workers get the persona
 * over Chromium's browser-level endpoint, read from DevToolsActivePort, and a
 * browser with no persona or no port yet is left alone. The endpoint is faked
 * with a real loopback HTTP and WebSocket server.
 */
const { describe, it, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { WebSocketServer } = require('ws');
const { WorkerCoverage, debugPort } = require('../../../../main/tabs/workers.cjs');

const PROFILE = {
  navigator: { platform: 'MacIntel', hardwareConcurrency: 8, deviceMemory: 8, languages: ['en-US', 'en'] },
  webgl: { unmaskedVendor: 'Apple', unmaskedRenderer: 'Apple M1' },
  timezone: 'America/New_York',
  locale: 'en-US',
};
const PERSONA = { screen: false, injection: {}, profile: PROFILE, userAgent: { userAgent: 'UA' } };

/** A browser endpoint that answers every command, and announces one service worker once auto-attach is on. */
async function fakeBrowser() {
  const calls = [];
  const server = http.createServer((req, res) => {
    const { port } = server.address();
    res.end(JSON.stringify({ webSocketDebuggerUrl: `ws://127.0.0.1:${port}/devtools/browser/x` }));
  });
  const wss = new WebSocketServer({ server });
  wss.on('connection', (ws) => ws.on('message', (data) => answer(ws, JSON.parse(String(data)), calls)));
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  return { calls, port: server.address().port, close: () => (wss.close(), server.close()) };
}

/** Records and answers one command; turning auto-attach on attaches a paused service worker. */
function answer(ws, msg, calls) {
  calls.push(msg);
  ws.send(JSON.stringify({ id: msg.id, result: {} }));
  if (msg.method !== 'Target.setAutoAttach') return;
  const params = { sessionId: 'sw1', targetInfo: { type: 'service_worker' }, waitingForDebugger: true };
  ws.send(JSON.stringify({ method: 'Target.attachedToTarget', params }));
}

/** A userData folder with DevToolsActivePort written as Chromium writes it. */
function userDataWith(port) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-workers-'));
  if (port) fs.writeFileSync(path.join(dir, 'DevToolsActivePort'), `${port}\n/devtools/browser/x`);
  return dir;
}

/** Resolves once `calls` holds a command for `method` on session `sessionId`. */
async function until(calls, method, sessionId) {
  for (let i = 0; i < 100 && !calls.some((c) => c.method === method && c.sessionId === sessionId); i++) {
    await new Promise((r) => setTimeout(r, 10));
  }
}

describe('WorkerCoverage', () => {
  let browser;
  let coverage;
  afterEach(() => {
    coverage?.stop();
    browser?.close();
  });

  it('reads the port Chromium wrote, and nothing before it has', () => {
    assert.equal(debugPort(userDataWith(9333)), 9333);
    assert.equal(debugPort(userDataWith(null)), null);
  });

  it('holds new service and shared workers, puts the persona on them, then lets them run', async () => {
    browser = await fakeBrowser();
    const dir = userDataWith(browser.port);
    coverage = new WorkerCoverage({ protection: { personaOptions: () => PERSONA } }, () => dir);
    await coverage.cover();
    await until(browser.calls, 'Runtime.runIfWaitingForDebugger', 'sw1');
    const autoAttach = browser.calls.find((c) => c.method === 'Target.setAutoAttach');
    assert.equal(autoAttach.params.waitForDebuggerOnStart, true);
    assert.deepEqual(autoAttach.params.filter.slice(0, 2), [{ type: 'service_worker' }, { type: 'shared_worker' }]);
    const onWorker = browser.calls.filter((c) => c.sessionId === 'sw1').map((c) => c.method);
    assert.ok(onWorker.includes('Network.setUserAgentOverride'), onWorker.join());
    assert.ok(onWorker.indexOf('Runtime.evaluate') < onWorker.indexOf('Runtime.runIfWaitingForDebugger'));
  });

  it('leaves the browser alone without a persona or before Chromium wrote its port', async () => {
    browser = await fakeBrowser();
    const noPersona = new WorkerCoverage({ protection: { personaOptions: () => null } }, () =>
      userDataWith(browser.port),
    );
    await noPersona.cover();
    const noPort = new WorkerCoverage({ protection: { personaOptions: () => PERSONA } }, () => userDataWith(null));
    await noPort.cover();
    assert.deepEqual(browser.calls, []);
  });

  it('never throws when the endpoint is gone', async () => {
    coverage = new WorkerCoverage({ protection: { personaOptions: () => PERSONA } }, () => userDataWith(1));
    await coverage.cover();
  });
});
