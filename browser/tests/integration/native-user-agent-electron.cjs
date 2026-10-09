/** First-script and HTTP user-agent isolation must come from the native session, never CDP or page shims. */
const { app, BrowserWindow, session } = require('electron');
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { once } = require('node:events');
const metadata = require('./native-user-agent-metadata.cjs');
const profile = process.env.OYA_USER_AGENT_PROFILE;
if (!profile) throw Error('Launch with native-user-agent.mjs for parent-owned cleanup');
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const windows = [];
const deadline = setTimeout(() => {
  console.error('Native user-agent fixture timed out');
  app.exit(1);
}, 60000);
/** Capture navigator and the actual main-script request header before page code runs. */
function serve(req, res) {
  if (req.url === '/echo') {
    res.setHeader('Content-Type', 'application/json');
    return res.end(JSON.stringify(req.headers['user-agent'] ?? ''));
  }
  res.setHeader('Content-Type', req.url.endsWith('.js') ? 'application/javascript' : 'text/html');
  const first = `${metadata.capture}const first={ua:navigator.userAgent,header:${JSON.stringify(req.headers['user-agent'] ?? '')}};`;
  const reply = `fetch('/echo').then(r=>r.json()).then(fetchHeader=>({...first,fetchHeader})).then(withMetadata)`;
  if (req.url === '/dedicated.js') return res.end(first + reply + '.then(value=>postMessage(value));');
  if (req.url === '/shared.js')
    return res.end(first + `onconnect=e=>${reply}.then(value=>e.ports[0].postMessage(value));`);
  if (req.url === '/sw.js')
    return res.end(
      first +
        `oninstall=()=>self.skipWaiting();onactivate=e=>e.waitUntil(clients.claim());onmessage=e=>e.waitUntil(${reply}.then(value=>e.ports[0].postMessage(value)));onfetch=e=>{if(new URL(e.request.url).pathname==='/worker-first')e.respondWith(${reply}.then(Response.json))};`,
    );
  res.end(
    `<!doctype html><script>${first}globalThis.first=first;onmessage=e=>${reply}.then(value=>e.source.postMessage(value,e.origin));</script>`,
  );
}
/** Real Oya surfaces throw immediately if any implementation attempts debugger access. */
function windowFor(jar) {
  const window = new BrowserWindow({
    show: false,
    webPreferences: { session: jar, sandbox: true, contextIsolation: true },
  });
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  windows.push(window);
  return window;
}
/** Read document, cross-origin frame and each worker's earliest observable identity and real fetch header. */
async function snapshots(window, url) {
  await window.loadURL(url);
  const read = (source) => window.webContents.executeJavaScript(source);
  const page = await read(
    `fetch('/echo').then(r=>r.json()).then(fetchHeader=>({...first,fetchHeader})).then(withMetadata)`,
  );
  const dedicated = await read(
    `new Promise((resolve,reject)=>{const w=new Worker('/dedicated.js');w.onmessage=e=>{w.terminate();resolve(e.data)};w.onerror=reject})`,
  );
  const shared = await read(
    `new Promise((resolve,reject)=>{const w=new SharedWorker('/shared.js');w.port.onmessage=e=>{w.port.close();resolve(e.data)};w.onerror=reject})`,
  );
  const service = await read(
    `(async()=>{await navigator.serviceWorker.register('/sw.js');const r=await navigator.serviceWorker.ready;return new Promise(resolve=>{const c=new MessageChannel();c.port1.onmessage=e=>{c.port1.close();resolve(e.data)};r.active.postMessage('read',[c.port2])})})()`,
  );
  const child = url.replace('127.0.0.1', 'localhost');
  const frame = await read(
    `new Promise(resolve=>{const f=document.createElement('iframe');f.src=${JSON.stringify(child)};window.addEventListener('message',e=>{if(e.source===f.contentWindow)resolve(e.data)},{once:true});f.onload=()=>f.contentWindow.postMessage('read','*');document.body.append(f)})`,
  );
  return { page, dedicated, shared, service, frame };
}
/** Every native surface and its network request must agree on this exact session, not another partition. */
async function check(window, url, ua, index) {
  for (const [surface, actual] of Object.entries(await snapshots(window, url)))
    assert.deepEqual(actual, { ua, header: ua, fetchHeader: ua, ...metadata.expected(index) }, surface);
}
/** Reject control characters, unbounded input and changing a frozen identity without mutating state. */
function validate(jar, ua) {
  assert.equal(typeof jar._setOyaUserAgent, 'function', 'patched native user-agent API required');
  for (const invalid of ['', 'a\r\nb', 'a\0b', 'a\tb', 'é', '\x7f', 'x'.repeat(1025)]) {
    assert.throws(() => jar._setOyaUserAgent(invalid), /Invalid native user agent/);
    assert.equal(jar._getOyaSessionPolicy().userAgent, '');
  }
  for (const invalid of [null, undefined, 42, true, {}]) assert.throws(() => jar._setOyaUserAgent(invalid));
  jar._setOyaUserAgent(ua);
  jar._setOyaUserAgent(ua);
  assert.throws(() => jar._setOyaUserAgent('Different/1'), /cannot be changed/);
  assert.throws(() => jar.setUserAgent('Different/1', 'fr-FR'), /owns User-Agent/);
  assert.equal(jar.getUserAgent(), ua);
  assert.equal(jar._getOyaSessionPolicy().userAgent, ua);
  const copy = jar._getOyaSessionPolicy();
  copy.userAgent = 'Forged/1';
  assert.equal(jar._getOyaSessionPolicy().userAgent, ua);
}
/** Public setters, per-navigation options and restored history cannot escape native ownership. */
async function protectedOverrides(window, jar, url, ua, index) {
  metadata.frozen(jar, index);
  jar.setUserAgent(ua);
  jar._setOyaUserAgent(ua);
  assert.throws(() => window.webContents.setUserAgent('Different/1'), /owns User-Agent/);
  window.webContents.setUserAgent(ua);
  await window.loadURL(url + 'identical', { userAgent: ua });
  await assert.rejects(
    window.loadURL(url + 'rejected', { userAgent: 'Different/1' }),
    /ERR_INVALID_ARGUMENT|Native session owns/,
  );
  assert.equal(window.webContents.getUserAgent(), ua);
  await check(window, url + 'next', ua, index);
  const restored = windowFor(jar);
  const entries = window.webContents.navigationHistory.getAllEntries();
  await restored.webContents.navigationHistory.restore({ entries, index: entries.length - 1 });
  assert.equal(await restored.webContents.executeJavaScript('navigator.userAgent'), ua);
  await check(restored, url + 'restored', ua, index);
}
/** Exercise actual parallel partitions, already-created network contexts and ownerless service-worker restart. */
async function run() {
  await app.whenReady();
  const site = createServer(serve);
  site.listen(0, '0.0.0.0');
  await once(site, 'listening');
  const url = `http://127.0.0.1:${site.address().port}/`;
  const fallback = app.userAgentFallback;
  try {
    await metadata.checkLifecycle(session, windowFor, url);
    const host = session.fromPartition('ua-host');
    const hostWindow = windowFor(host);
    const baseline = await snapshots(hostWindow, url);
    assert.throws(() => host._setOyaUserAgent('Late/1'), /before any session renderer/);
    hostWindow.destroy();
    assert.throws(() => host._setOyaUserAgent('Late/1'), /before any session renderer/);
    const bounded = session.fromPartition('ua-maximum');
    bounded._setOyaUserAgent('x'.repeat(1024));
    assert.equal(await (await bounded.fetch(url + 'echo')).json(), 'x'.repeat(1024));
    const jars = [session.fromPartition('ua-one'), session.fromPartition('ua-two')];
    const values = ['OyaFixture/1 SessionOne', 'OyaFixture/1 SessionTwo'];
    for (const [index, jar] of jars.entries()) {
      await (await jar.fetch(url + 'echo')).json();
      validate(jar, values[index]);
      metadata.configure(jar, index);
      assert.equal(await (await jar.fetch(url + 'echo')).json(), values[index]);
    }
    const surfaces = jars.map(windowFor);
    await Promise.all(surfaces.map((window, index) => check(window, url, values[index], index)));
    app.userAgentFallback = 'ApplicationWide/9';
    for (const [index, jar] of jars.entries()) {
      await protectedOverrides(surfaces[index], jar, url, values[index], index);
      for (const window of windows) if (!window.isDestroyed() && window.webContents.session === jar) window.destroy();
      await jar.serviceWorkers._stopAllWorkers();
      await jar.serviceWorkers.startWorkerForScope(url);
      const resumed = windowFor(jar);
      await resumed.loadURL(url);
      assert.deepEqual(await resumed.webContents.executeJavaScript('fetch("/worker-first").then(r=>r.json())'), {
        ua: values[index],
        header: values[index],
        fetchHeader: values[index],
        ...metadata.expected(index),
      });
      await check(resumed, url, values[index], index);
    }
    app.userAgentFallback = fallback;
    const unchanged = windowFor(session.fromPartition('ua-unchanged'));
    assert.deepEqual(await snapshots(unchanged, url), baseline, 'unconfigured host behavior preserved');
    if (metadata.enabled)
      console.log(
        'PASS native user-agent metadata: low/high entropy, first scripts, all worker types, immutable lifecycle, snapshot isolation and cold restart',
      );
    console.log(
      'PASS native user-agent strings: first scripts, headers/fetches, frames, all worker types, parallel sessions, immutable overrides, history and cold worker restart',
    );
  } finally {
    app.userAgentFallback = fallback;
    for (const window of windows) if (!window.isDestroyed()) window.destroy();
    site.closeAllConnections();
    await new Promise((resolve) => site.close(resolve));
  }
}
run().then(
  () => {
    clearTimeout(deadline);
    app.exit(0);
  },
  (error) => {
    console.error(error);
    clearTimeout(deadline);
    app.exit(1);
  },
);
