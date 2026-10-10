/**
 * Real Electron regression for the identity a persona presents: what a page's
 * JavaScript reads (navigator.userAgent, navigator.userAgentData) must be what
 * the request headers say. This checks consistency, not Google's acceptance.
 * Native session policy installs before renderer creation. Virtual passkeys use
 * an explicitly enabled, ephemeral, browser-owned test authenticator, never CDP;
 * real-account and physical-passkey checks are separate.
 * npm run test:identity --prefix browser (Linux CI uses xvfb-run).
 */
const { app, BrowserWindow, BrowserView, session } = require('electron');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { createServer } = require('node:https');
const { execFileSync } = require('node:child_process');
const { generateProfile } = require('../../anonymity/fingerprint');
const { configureSession } = require('../../src/main/identity/session.ts');
const { applyTelemetryFlags } = require('../../src/anonymity/telemetry.ts');
const { applyDNSLeakPrevention } = require('../../src/anonymity/proxy.ts');
const { Protection } = require('../../src/main/tabs/protection.ts');
const { Dialogs } = require('../../src/main/cdp/dialogs.ts');
const { Governance } = require('../../src/main/identity/governance.ts');

/** How long the whole run may take. */
const TIMEOUT_MS = 90_000;
/** Every client hint a sign-in page may ask for. */
const ACCEPT_CH =
  'Sec-CH-UA-Full-Version-List, Sec-CH-UA-Platform-Version, Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Model, Sec-CH-UA-WoW64';
/** What the page reads about itself, high-entropy hints included. */
const READ_IDENTITY = `(async () => ({
  userAgent: navigator.userAgent,
  platform: navigator.platform,
  low: { brands: navigator.userAgentData.brands, platform: navigator.userAgentData.platform },
  high: await navigator.userAgentData.getHighEntropyValues(
    ['fullVersionList', 'platformVersion', 'architecture', 'bitness', 'model', 'wow64', 'uaFullVersion']),
}))()`;

/** The analyzer's isolated world plays no part in the identity. */
const NO_WORLD = { ensure: async () => {} };

// An exception in a main-process listener opens Electron's modal error box, which hangs the run.
process.on('uncaughtException', (error) => (console.error(error), app.exit(1)));

const userData = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-identity-electron-'));
app.setPath('userData', userData);
app.commandLine.appendSwitch('disable-gpu');
// The switches src/main/main.ts starts with: some of the surface (window.sharedStorage) is decided by them.
applyTelemetryFlags(app);
applyDNSLeakPrevention(app);
// Chrome sends client hints over TLS only; the fixture's certificate is self-signed.
app.commandLine.appendSwitch('ignore-certificate-errors');
app.once('quit', () => fs.rmSync(userData, { recursive: true, force: true }));

/** A Sec-CH-UA brand list header as `[{ brand, version }]`. */
function parseBrands(header) {
  return [...String(header || '').matchAll(/"([^"]*)";v="([^"]*)"/g)].map(([, brand, version]) => ({ brand, version }));
}

/** A throwaway self-signed certificate for localhost, made by the system's openssl. */
function selfSigned() {
  const [key, cert] = ['key.pem', 'cert.pem'].map((name) => path.join(userData, name));
  const subject = ['-subj', '/CN=localhost', '-days', '1'];
  execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', key, '-out', cert, ...subject], {
    stdio: 'ignore',
  });
  return { key: fs.readFileSync(key), cert: fs.readFileSync(cert) };
}

/** A server that asks for every hint and remembers the headers of each page request. */
async function hintServer() {
  const seen = [];
  const server = createServer(selfSigned(), (req, res) => {
    if (req.url.startsWith('/page')) seen.push(req.headers);
    res.writeHead(200, { 'Content-Type': 'text/html', 'Accept-CH': ACCEPT_CH });
    res.end('<!doctype html><title>identity</title>ok');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  return { server, seen, url: (p) => `https://localhost:${server.address().port}${p}` };
}

/** A tab set up the way the app sets one up: the persona's session, then its protection. */
async function personaTab(win, profile) {
  const partition = `identity-${profile.navigator.platform}`;
  // Ungoverned, as a person's own browser is.
  const governance = new Governance(null);
  const ses = session.fromPartition(partition);
  await configureSession(app, ses, profile, { governance });
  const protection = new Protection({
    persona: { active: profile },
    world: NO_WORLD,
    dialogs: new Dialogs(),
    governance,
  });
  protection.configureSession(ses);
  const view = new BrowserView({ webPreferences: { contextIsolation: true, sandbox: true, partition } });
  forbidDebugger(view.webContents);
  win.setBrowserView(view);
  view.setBounds({ x: 0, y: 0, width: 800, height: 600 });
  await view.webContents.loadURL('about:blank');
  assert.equal(await protection.setupTabCDP(view), true);
  return view;
}

/** Any debugger access fails this native-only fixture immediately. */
function forbidDebugger(contents) {
  Object.defineProperty(contents, 'debugger', {
    get() {
      throw Error('Internal CDP is forbidden');
    },
  });
}

/** Host-only native policy preserves the host platform while reducing the engine version consistently. */
async function checkNativeBrowsing(win, site) {
  const partition = 'native-desktop-identity';
  const ses = session.fromPartition(partition);
  const original = ses.getUserAgent();
  const expected = original
    .replace(/Chrome\/[\d.]+/, `Chrome/${process.versions.chrome.split('.')[0]}.0.0.0`)
    .replace(/ Electron\/[\d.]+/, '');
  await configureSession(app, ses, null, { nativeBrowsing: true });
  const protection = new Protection({
    nativeBrowsing: true,
    persona: { active: null },
    governance: new Governance(null),
    dialogs: new Dialogs(),
  });
  protection.configureSession(ses);
  const view = new BrowserView({ webPreferences: { contextIsolation: true, sandbox: true, partition } });
  forbidDebugger(view.webContents);
  win.setBrowserView(view);
  view.setBounds({ x: 0, y: 0, width: 800, height: 600 });
  await view.webContents.loadURL('about:blank');
  assert.equal(await protection.setupTabCDP(view), true);
  await view.webContents.loadURL(site.url('/page?native'));
  await protection.injectScripts(view, true);
  assert.throws(() => view.webContents._setOyaTestAuthenticator(true), /test opt-in/);
  const ua = await view.webContents.executeJavaScript('navigator.userAgent', true);
  assert.equal(ua, expected, 'page presents the reduced host-native identity');
  assert.equal(site.seen.at(-1)['user-agent'], expected, 'requests match the native session identity');
  win.removeBrowserView(view);
  view.webContents.close();
}

/** Everything a page says about itself must be what its request said. */
function assertConsistent(platform, page, headers) {
  const where = `${platform}: `;
  assert.equal(page.userAgent, headers['user-agent'], where + 'navigator.userAgent is the User-Agent header');
  assert.match(page.userAgent, /Chrome\/\d+\.0\.0\.0 /, where + 'the user agent is reduced, as real Chrome sends it');
  assert.doesNotMatch(page.userAgent, /Electron|oya/i, where + 'the user agent hides Electron');
  assert.equal(page.platform, platform, where + 'navigator.platform is the persona platform');
  assert.deepEqual(page.low.brands, parseBrands(headers['sec-ch-ua']), where + 'brands are the Sec-CH-UA header');
  assert.ok(
    page.low.brands.some((b) => b.brand === 'Google Chrome'),
    where + 'JavaScript sees Google Chrome, not bare Chromium',
  );
  assert.equal(`"${page.low.platform}"`, headers['sec-ch-ua-platform'], where + 'platform hint');
  assertHighEntropy(where, page.high, headers);
}

/** The hints a site has to ask for agree too, and name the engine's real version. */
function assertHighEntropy(where, high, headers) {
  assert.deepEqual(high.fullVersionList, parseBrands(headers['sec-ch-ua-full-version-list']), where + 'full versions');
  assert.equal(`"${high.platformVersion}"`, headers['sec-ch-ua-platform-version'], where + 'platform version');
  assert.equal(`"${high.architecture}"`, headers['sec-ch-ua-arch'], where + 'architecture');
  assert.equal(`"${high.bitness}"`, headers['sec-ch-ua-bitness'], where + 'bitness');
  const chrome = high.fullVersionList.find((b) => b.brand === 'Google Chrome')?.version;
  assert.equal(chrome, process.versions.chrome, where + 'the claimed Chrome is the engine that is running');
}

/** Exercise native WebAuthn with an isolated virtual authenticator, never a real user's credentials. */
const PASSKEY_ROUNDTRIP = `(async () => {
  const created = await navigator.credentials.create({ publicKey: {
    challenge: new Uint8Array(32), rp: { name: 'Oya test', id: 'localhost' },
    user: { id: new Uint8Array([1]), name: 'test', displayName: 'Test account' },
    pubKeyCredParams: [{ type: 'public-key', alg: -7 }], timeout: 5000,
    authenticatorSelection: { residentKey: 'required', userVerification: 'required' }
  }});
  const signed = await navigator.credentials.get({ publicKey: {
    challenge: new Uint8Array(32), rpId: 'localhost', timeout: 5000,
    allowCredentials: [{ type: 'public-key', id: created.rawId }], userVerification: 'required'
  }});
  const rejected = (promise) => promise.then(() => 'accepted', (error) => error.name);
  const invalidRP = await rejected(navigator.credentials.get({ publicKey: {
    challenge: new Uint8Array(32), rpId: 'https://localhost', timeout: 5000
  }}));
  return { same: created.id === signed.id, signature: signed.response.signature.byteLength, invalidRP };
})()`;

/** Keep synthetic user presence pending so cancellation cannot race an instant successful assertion. */
const PASSKEY_ABORT = `(async () => {
  const controller = new AbortController();
  const pending = navigator.credentials.get({ signal: controller.signal, publicKey: {
    challenge: new Uint8Array(32), rpId: 'localhost', timeout: 5000, userVerification: 'required'
  }});
  controller.abort();
  return pending.then(() => 'accepted', (error) => error.name);
})()`;

/** Registration and assertion must reach Chromium instead of the old forced NotAllowedError. */
async function assertNativePasskeys(view, where) {
  view.webContents._setOyaTestAuthenticator(true);
  assert.throws(() => view.webContents._setOyaTestAuthenticator(true), /already enabled/);
  try {
    const foreign = new BrowserView({ webPreferences: { sandbox: true, partition: 'foreign-authenticator-test' } });
    await foreign.webContents.loadURL('about:blank');
    assert.throws(() => foreign.webContents._setOyaTestAuthenticator(false), /does not own/);
    assert.throws(() => foreign.webContents._setOyaTestAuthenticatorPresence(false), /does not own/);
    foreign.webContents.close();
    const result = await view.webContents.executeJavaScript(PASSKEY_ROUNDTRIP, true);
    assert.equal(result.same, true, where + 'native registration and assertion use the same credential');
    assert.ok(result.signature > 0, where + 'native assertion returns a cryptographic signature');
    assert.equal(result.invalidRP, 'SecurityError', where + 'invalid RP identifiers remain rejected');
    view.webContents._setOyaTestAuthenticatorPresence(false);
    assert.equal(
      await view.webContents.executeJavaScript(PASSKEY_ABORT, true),
      'AbortError',
      where + 'aborting a pending assertion reaches native cancellation',
    );
  } finally {
    view.webContents._setOyaTestAuthenticator(false);
  }
  assert.throws(() => view.webContents._setOyaTestAuthenticator(false), /does not own/);
}

/** What a page and its worker can learn without asking the person anything, as a fresh Chrome answers it. */
const READ_SURFACE = `(async () => {
  const state = (name) => navigator.permissions.query({ name }).then((p) => p.state, (e) => 'ERR ' + e.name);
  const inWorker = new Promise((resolve) => {
    const code = 'navigator.permissions.query({ name: "notifications" }).then((p) => postMessage([p.state, navigator.languages]))';
    const worker = new Worker(URL.createObjectURL(new Blob([code])));
    worker.onmessage = (e) => resolve(e.data);
    setTimeout(() => resolve(['no answer']), 4000);
  });
  const devices = await navigator.mediaDevices.enumerateDevices();
  return {
    states: { notifications: await state('notifications'), camera: await state('camera'), microphone: await state('microphone'), geolocation: await state('geolocation'), midi: await state('midi') },
    notification: Notification.permission,
    worker: await inWorker,
    languages: navigator.languages,
    devices: devices.map((d) => d.kind + ':' + (d.label ? 'labelled' : '')).sort(),
    share: 'share' in navigator && 'canShare' in navigator,
    sharedStorage: 'sharedStorage' in window,
  };
})()`;

/**
 * Electron grants every permission unasked, which no browser does, and which handed any
 * page the camera. Sensitive ones are refused natively, and read as Chrome's "prompt".
 */
function assertFreshChromeSurface(where, surface, profile, headers) {
  for (const [name, state] of Object.entries(surface.states))
    assert.equal(state, 'prompt', where + name + ' reads as not yet asked');
  assert.equal(surface.notification, 'default', where + 'Notification.permission');
  assert.deepEqual(
    surface.worker,
    ['prompt', profile.navigator.languages],
    where + 'a worker says the same as its page',
  );
  assert.deepEqual(surface.languages, profile.navigator.languages, where + 'navigator.languages is the persona list');
  assert.match(
    headers['accept-language'],
    /^[a-z]{2}-[A-Z]{2},[a-z]{2};q=0\.9/,
    where + 'Accept-Language carries the list, as Chrome writes it',
  );
  assert.deepEqual(
    surface.devices,
    ['audioinput:', 'audiooutput:', 'videoinput:'],
    where + 'devices stay anonymous until permission',
  );
  assert.ok(surface.share, where + 'navigator.share exists, as in Chrome');
  assert.equal(surface.sharedStorage, false, where + 'no sharedStorage, as in Chrome');
}

/** One persona platform: load once so the hints are asked for, again so they are sent, then compare. */
async function checkPlatform(win, site, platform) {
  const profile = generateProfile({ seed: `identity-${platform}`, platform });
  const view = await personaTab(win, profile);
  await view.webContents.loadURL(site.url('/warm'));
  await view.webContents.loadURL(site.url(`/page?${platform}`));
  const page = await view.webContents.executeJavaScript(READ_IDENTITY, true);
  assertConsistent(platform, page, site.seen.at(-1));
  await assertNativePasskeys(view, `${platform}: `);
  const surface = await view.webContents.executeJavaScript(READ_SURFACE, true);
  assertFreshChromeSurface(`${platform}: `, surface, profile, site.seen.at(-1));
  win.removeBrowserView(view);
  view.webContents.close();
}

(async () => {
  await app.whenReady();
  setTimeout(() => (console.error('Electron identity test timed out'), app.exit(1)), TIMEOUT_MS).unref();
  const site = await hintServer();
  const win = new BrowserWindow({ show: true, width: 820, height: 640 });
  await checkNativeBrowsing(win, site);
  app.commandLine.appendSwitch('oya-test-virtual-authenticator');
  assert.throws(() => win.webContents._setOyaTestAuthenticator(true), /ephemeral session/);
  for (const platform of ['Win32', 'MacIntel', 'Linux x86_64']) await checkPlatform(win, site, platform);
  site.server.close();
  console.log(
    'Oya identity: host/persona page and request identity agree without CDP; owned ephemeral native virtual WebAuthn registration/assertion pass (not physical passkey verification)',
  );
  app.exit(0);
})().catch((error) => {
  console.error(error);
  app.exit(1);
});
