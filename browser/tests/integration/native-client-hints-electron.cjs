/** Native client-hint negotiation must preserve session/origin boundaries, opt-in and permissions policy. */
const { app, BrowserWindow, session } = require('electron');
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { once } = require('node:events');
const workers = require('./native-worker-client-hints.cjs');
const profile = process.env.OYA_CLIENT_HINTS_PROFILE;
if (!profile) throw Error('Launch with native-client-hints.mjs for parent-owned profile cleanup');
app.setPath('userData', profile);
app.on('window-all-closed', () => {});
app.commandLine.appendSwitch('site-per-process');
app.commandLine.appendSwitch('host-resolver-rules', 'MAP insecure.oya.test 127.0.0.1');
const windows = [];
const requests = [];
const high = {
  'sec-ch-ua-arch': '"arm"',
  'sec-ch-ua-bitness': '"64"',
  'sec-ch-ua-model': '""',
  'sec-ch-ua-platform-version': '"10.2.0"',
  'sec-ch-ua-full-version': '"1.2.3.4"',
  'sec-ch-ua-full-version-list': '"OyaFixture";v="1.2.3.4"',
  'sec-ch-ua-wow64': '?0',
  'sec-ch-ua-form-factors': '"Desktop"',
};
const low = { 'sec-ch-ua': '"OyaFixture";v="1"', 'sec-ch-ua-mobile': '?0', 'sec-ch-ua-platform': '"FixtureOS"' };
const metadata = {
  brands: [{ brand: 'OyaFixture', version: '1' }],
  fullVersionList: [{ brand: 'OyaFixture', version: '1.2.3.4' }],
  fullVersion: '1.2.3.4',
  platform: 'FixtureOS',
  platformVersion: '10.2.0',
  architecture: 'arm',
  bitness: '64',
  model: '',
  mobile: false,
  wow64: false,
  formFactors: ['Desktop'],
};
const deadline = setTimeout(() => {
  console.error('Native client-hint fixture timed out');
  app.exit(1);
}, 60000);
/** Capture actual requests and use ordinary HTTP response headers, not request interception. */
function serve(req, res) {
  const path = new URL(req.url, 'http://local').pathname;
  requests.push({ path, headers: req.headers });
  res.setHeader('Cache-Control', 'no-store');
  if (path === '/echo') {
    res.setHeader('Content-Type', 'application/json');
    res.setHeader('Access-Control-Allow-Origin', '*');
    return res.end(JSON.stringify(req.headers));
  }
  if (workers.serve(req, res, path)) return;
  if (path === '/opt' || path === '/disabled' || path === '/critical')
    res.setHeader('Accept-CH', Object.keys(high).join(', '));
  if (path === '/critical') res.setHeader('Critical-CH', 'Sec-CH-UA-Arch');
  if (path === '/redirect') {
    res.writeHead(302, { Location: `http://localhost:${req.socket.localPort}/redirected` });
    return res.end();
  }
  if (path === '/empty') res.setHeader('Accept-CH', '');
  if (path === '/clear') res.setHeader('Clear-Site-Data', '"clientHints"');
  if (path === '/deny')
    res.setHeader('Permissions-Policy', 'ch-ua-arch=(), ch-ua-platform-version=(), ch-ua-full-version-list=()');
  if (path === '/delegate')
    res.setHeader('Permissions-Policy', `ch-ua-arch=(self "http://localhost:${req.socket.localPort}")`);
  res.setHeader('Content-Type', 'text/html');
  res.end(
    `<!doctype html><script>globalThis.first=${JSON.stringify(req.headers)};onmessage=e=>e.source.postMessage(first,e.origin)</script>`,
  );
}
/** Each partition gets an immutable native identity before any window or worker starts. */
function jarFor(name, identity = metadata) {
  const jar = session.fromPartition(name);
  jar._setOyaUserAgent('OyaFixture/1 ' + name);
  jar._setOyaUserAgentMetadata(identity);
  return jar;
}
/** Refuse any debugger use and optionally disable actual page JavaScript through native preferences. */
function windowFor(jar, javascript = true) {
  const window = new BrowserWindow({
    show: false,
    webPreferences: { session: jar, javascript, sandbox: true, contextIsolation: true },
  });
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  windows.push(window);
  return window;
}
/** The server's first request headers are read only after navigation finishes. */
async function visit(window, url) {
  await window.loadURL(url);
  return window.webContents.executeJavaScript('first');
}
/** Low-entropy headers always use the native session metadata when JavaScript is enabled. */
function checkLow(headers) {
  for (const [key, value] of Object.entries(low)) assert.equal(headers[key], value, key);
}
/** Every high-entropy field must either match its native value or be absent without opt-in. */
function checkHigh(headers, enabled) {
  for (const [key, value] of Object.entries(high)) assert.equal(headers[key], enabled ? value : undefined, key);
}
/** Cross-origin frames report actual navigation headers without same-origin DOM access. */
async function frameHeaders(window, url, delegated = false) {
  return window.webContents.executeJavaScript(
    `new Promise(resolve=>{const f=document.createElement('iframe');${delegated ? "f.allow='ch-ua-arch';" : ''}f.src=${JSON.stringify(url)};window.addEventListener('message',e=>{if(e.source===f.contentWindow)resolve(e.data)},{once:true});f.onload=()=>f.contentWindow.postMessage('read','*');document.body.append(f)})`,
  );
}
/** Clearing browser data revokes opt-ins but must never mutate the immutable session identity. */
async function clearing(window, jar, url) {
  for (const clear of [
    () => jar.clearStorageData(),
    () => jar.clearCache(),
    () => jar.clearData({ dataTypes: ['cookies'] }),
  ]) {
    await visit(window, url + '/opt');
    checkHigh(await visit(window, url + '/before-clear'), true);
    await clear();
    checkHigh(await visit(window, url + '/after-clear'), false);
    assert.deepEqual(jar._getOyaSessionPolicy().userAgentMetadata, metadata);
  }
}
/** Real opt-in, denial, revocation and independent partitions exercise the browser-owned native controller. */
async function run() {
  await app.whenReady();
  const sites = [createServer(serve), createServer(serve)];
  for (const site of sites) {
    site.listen(0, '0.0.0.0');
    await once(site, 'listening');
  }
  const url = `http://127.0.0.1:${sites[0].address().port}`;
  const otherPort = `http://127.0.0.1:${sites[1].address().port}`;
  const cross = url.replace('127.0.0.1', 'localhost');
  try {
    const jar = jarFor('hints-one');
    await jar.setProxy({ mode: 'direct' });
    const window = windowFor(jar);
    const first = await visit(window, url + '/first');
    checkLow(first);
    checkHigh(first, false);
    checkHigh(await visit(window, url + '/opt'), false);
    const opted = await visit(window, url + '/opted');
    checkLow(opted);
    checkHigh(opted, true);
    checkHigh(await window.webContents.executeJavaScript('fetch("/echo").then(r=>r.json())'), true);
    checkHigh(await visit(window, otherPort + '/port-isolation'), false);
    checkHigh(await visit(window, cross + '/host-isolation'), false);
    checkHigh(await visit(window, url + '/redirect'), false);
    assert.equal(window.webContents.getURL(), cross + '/redirected');
    checkHigh(requests.findLast((entry) => entry.path === '/redirect').headers, true);
    const insecure = url.replace('127.0.0.1', 'insecure.oya.test');
    await visit(window, insecure + '/opt');
    const insecureHeaders = await visit(window, insecure + '/insecure');
    for (const key of [...Object.keys(low), ...Object.keys(high)]) assert.equal(insecureHeaders[key], undefined, key);
    const sibling = windowFor(jarFor('hints-two', { ...metadata, platform: 'OtherFixtureOS', architecture: 'x86' }));
    const siblingFirst = await visit(sibling, url + '/session-isolation');
    checkHigh(siblingFirst, false);
    assert.equal(siblingFirst['sec-ch-ua-platform'], '"OtherFixtureOS"');
    await visit(sibling, url + '/opt');
    assert.equal((await visit(sibling, url + '/other-metadata'))['sec-ch-ua-arch'], '"x86"');
    const replacement = windowFor(jar);
    checkHigh(await visit(replacement, url + '/same-session'), true);
    await visit(window, url + '/deny');
    const denied = await window.webContents.executeJavaScript('fetch("/echo").then(r=>r.json())');
    for (const key of ['sec-ch-ua-arch', 'sec-ch-ua-platform-version', 'sec-ch-ua-full-version-list'])
      assert.equal(denied[key], undefined, key);
    await visit(window, url + '/parent');
    assert.equal((await frameHeaders(window, cross + '/child-denied'))['sec-ch-ua-arch'], undefined);
    await visit(window, url + '/delegate');
    assert.equal(
      (await frameHeaders(window, cross + '/child-allowed', true))['sec-ch-ua-arch'],
      high['sec-ch-ua-arch'],
    );
    await visit(window, url + '/empty');
    checkHigh(await visit(window, url + '/after-empty'), false);
    await visit(window, url + '/opt');
    await visit(window, url + '/clear');
    checkHigh(await visit(window, url + '/after-clear-site'), false);
    await clearing(window, jar, url);
    assert.equal((await visit(sibling, url + '/after-other-session-clear'))['sec-ch-ua-arch'], '"x86"');
    const embedded = windowFor(jarFor('hints-subframe'));
    await visit(embedded, url + '/embedding');
    await frameHeaders(embedded, cross + '/opt');
    checkHigh(await visit(embedded, cross + '/after-subframe-opt'), false);
    checkHigh(await visit(embedded, url + '/after-subframe-opt'), false);
    const criticalJar = jarFor('hints-critical');
    const critical = await visit(windowFor(criticalJar), url + '/critical');
    checkHigh(critical, true);
    const retries = requests.filter(
      (entry) => entry.path === '/critical' && entry.headers['user-agent'] === criticalJar.getUserAgent(),
    );
    assert.equal(retries.length, 2, 'Critical-CH performs one native retry');
    checkHigh(retries[0].headers, false);
    checkHigh(retries[1].headers, true);
    const disabledJar = jarFor('hints-disabled');
    const disabled = windowFor(disabledJar, false);
    await disabled.loadURL(url + '/disabled');
    const disabledRequest = requests.findLast((entry) => entry.path === '/disabled').headers;
    for (const key of [...Object.keys(low), ...Object.keys(high)]) assert.equal(disabledRequest[key], undefined, key);
    checkHigh(await visit(windowFor(disabledJar), url + '/after-disabled'), false);
    const unconfigured = windowFor(session.fromPartition('hints-legacy'));
    const legacy = await visit(unconfigured, url + '/legacy');
    for (const key of [...Object.keys(low), ...Object.keys(high)]) assert.equal(legacy[key], undefined, key);
    await workers.verify({ jarFor, windowFor, visit, url, metadata });
    console.log(
      'PASS native client hints: low/high metadata headers, origin opt-in, host/port/session isolation, permissions-policy denial/delegation, insecure origins, redirects, Critical-CH retry, JavaScript disabled, empty Accept-CH, Clear-Site-Data and native data clearing',
    );
  } finally {
    for (const window of windows) if (!window.isDestroyed()) window.destroy();
    for (const site of sites) {
      site.closeAllConnections();
      await new Promise((resolve) => site.close(resolve));
    }
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
