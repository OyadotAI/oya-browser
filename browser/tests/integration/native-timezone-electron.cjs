/** Native session timezones must precede first page/worker execution and remain isolated across partitions. */
const { app, BrowserWindow, session } = require('electron');
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { once } = require('node:events');
const profile = process.env.OYA_TIMEZONE_PROFILE;
if (!profile) throw Error('Launch through native-timezone.mjs so cleanup happens after Electron exits');
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const deadline = setTimeout(() => {
  console.error('Native timezone fixture timed out');
  app.exit(1);
}, 60000);
const locale = process.env.OYA_CHECK_NATIVE_LOCALE === '1';
const localeFields = locale
  ? ',locale:Intl.DateTimeFormat().resolvedOptions().locale,numberLocale:Intl.NumberFormat().resolvedOptions().locale,collatorLocale:Intl.Collator().resolvedOptions().locale,number:Intl.NumberFormat().format(1234.5),lower:"I".toLocaleLowerCase(),lowerEmpty:"I".toLocaleLowerCase([]),lowerEnglish:"I".toLocaleLowerCase("en-US"),upper:"i".toLocaleUpperCase(),upperEmpty:"i".toLocaleUpperCase([]),upperEnglish:"i".toLocaleUpperCase("en-US")'
  : '';
const hardware = process.env.OYA_CHECK_NATIVE_HARDWARE === '1';
const extraSnapshot = hardware ? ',cores:navigator.hardwareConcurrency' : '';
const snapshot = `({zone:Intl.DateTimeFormat().resolvedOptions().timeZone,offset:new Date('2020-01-01T00:00:00Z').getTimezoneOffset(),hour:new Date('2020-01-01T00:00:00Z').getHours()${extraSnapshot}${localeFields}})`;
/** Every snapshot is captured before the first message or other page action can change worker state. */
function serve(req, res) {
  res.setHeader('Content-Type', req.url.endsWith('.js') ? 'application/javascript' : 'text/html');
  const start = `const first=${snapshot};`;
  if (req.url === '/dedicated.js') return res.end(start + 'postMessage(first);');
  if (req.url === '/shared.js') return res.end(start + 'onconnect=e=>e.ports[0].postMessage(first);');
  if (req.url === '/sw.js')
    return res.end(
      start +
        `oninstall=()=>self.skipWaiting();onactivate=e=>e.waitUntil(clients.claim());onmessage=e=>e.ports[0].postMessage(first);onfetch=e=>{if(new URL(e.request.url).pathname==='/worker-first')e.respondWith(Response.json(first))};`,
    );
  res.end(`<!doctype html><script>globalThis.first=${snapshot};</script><title>Native timezone</title>`);
}
/** Each surface refuses any implicit debugger access. */
function windowFor(
  jar,
  additionalArguments = [
    '--oya-session-timezone=Pacific/Honolulu',
    '--oya-session-hardware-concurrency=99',
    '--oya-session-locale=fr-FR',
  ],
) {
  const window = new BrowserWindow({
    show: false,
    webPreferences: {
      session: jar,
      sandbox: true,
      contextIsolation: true,
      additionalArguments,
    },
  });
  Object.defineProperty(window.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  return window;
}
/** Actual native Date arithmetic and first-script Intl values must agree in all worker types. */
async function check(window, url, expected) {
  await window.loadURL(url);
  const evaluate = (source) => window.webContents.executeJavaScript(source);
  assert.deepEqual(await evaluate('first'), expected);
  assert.deepEqual(
    await evaluate(
      `new Promise((resolve,reject)=>{const w=new Worker('/dedicated.js');w.onmessage=e=>{w.terminate();resolve(e.data)};w.onerror=reject})`,
    ),
    expected,
  );
  assert.deepEqual(
    await evaluate(
      `new Promise((resolve,reject)=>{const w=new SharedWorker('/shared.js');w.port.onmessage=e=>{w.port.close();resolve(e.data)};w.onerror=reject})`,
    ),
    expected,
  );
  assert.deepEqual(
    await evaluate(
      `(async()=>{await navigator.serviceWorker.register('/sw.js');const registration=await navigator.serviceWorker.ready;return new Promise(resolve=>{const channel=new MessageChannel();channel.port1.onmessage=e=>{channel.port1.close();resolve(e.data)};registration.active.postMessage('read',[channel.port2])})})()`,
    ),
    expected,
  );
  const child = url.replace('127.0.0.1', 'localhost');
  assert.deepEqual(
    await evaluate(
      `new Promise(resolve=>{const f=document.createElement('iframe');f.src=${JSON.stringify(child)};window.addEventListener('message',e=>{if(e.source===f.contentWindow)resolve(e.data)},{once:true});f.onload=()=>f.contentWindow.postMessage('read','*');document.body.append(f)})`,
    ),
    expected,
  );
}
/** Invalid input never mutates policy; only an identical count may be repeated. */
function configureHardware(jar, cores) {
  assert.equal(typeof jar._setOyaHardwareConcurrency, 'function', 'patched native hardware API required');
  for (const invalid of [0, -1, 1.5, 257, NaN, Infinity, -Infinity])
    assert.throws(() => jar._setOyaHardwareConcurrency(invalid), /Invalid native hardware/);
  for (const invalid of ['8', true, null, undefined, {}]) assert.throws(() => jar._setOyaHardwareConcurrency(invalid));
  jar._setOyaHardwareConcurrency(cores);
  jar._setOyaHardwareConcurrency(cores);
  assert.throws(() => jar._setOyaHardwareConcurrency(cores === 1 ? 2 : 1), /cannot be changed/);
}
/** Unconfigured sessions preserve the host value and reject late installation, even after workers exist. */
async function unconfiguredHardware(jar, window, url, windows) {
  const baseline = windowFor(session.fromPartition('hardware-baseline'), []);
  windows.push(baseline);
  await baseline.loadURL(url);
  const host = await baseline.webContents.executeJavaScript('navigator.hardwareConcurrency');
  assert.ok(Number.isInteger(host) && host > 0);
  assert.equal(await window.webContents.executeJavaScript('navigator.hardwareConcurrency'), host);
  assert.throws(() => jar._setOyaHardwareConcurrency(8), /before any session renderer/);
  const own = session.fromPartition('hardware-only');
  own._setOyaHardwareConcurrency(4);
  const isolated = windowFor(own);
  windows.push(isolated);
  await check(isolated, url, { ...(await baseline.webContents.executeJavaScript('first')), cores: 4 });
  assert.throws(() => own._setOyaTimeZone('UTC'), /before any session renderer/);
  isolated.destroy();
  await own.serviceWorkers._stopAllWorkers();
  await own.serviceWorkers.startWorkerForScope(url);
  const resumed = windowFor(own);
  windows.push(resumed);
  await resumed.loadURL(url);
  assert.deepEqual(await resumed.webContents.executeJavaScript('fetch("/worker-first").then(reply=>reply.json())'), {
    ...(await baseline.webContents.executeJavaScript('first')),
    cores: 4,
  });
}
/** Expected ICU formatting is checked alongside resolved metadata, not just a claimed locale. */
function localeExpected(tag) {
  if (!locale) return {};
  return {
    locale: tag,
    numberLocale: tag,
    collatorLocale: tag,
    number: tag === 'en-US' ? '1,234.5' : '1.234,5',
    lower: tag === 'tr-TR' ? 'ı' : 'i',
    lowerEmpty: tag === 'tr-TR' ? 'ı' : 'i',
    lowerEnglish: 'i',
    upper: tag === 'tr-TR' ? 'İ' : 'I',
    upperEmpty: tag === 'tr-TR' ? 'İ' : 'I',
    upperEnglish: 'I',
  };
}
/** Partial or malformed language tags must fail before a session policy is installed. */
function configureLocale(jar, tag) {
  assert.equal(typeof jar._setOyaLocale, 'function', 'patched native locale API required');
  for (const invalid of ['', 'en_US', 'en-US-!', 'en-US\0hidden', 'é', 'x'.repeat(129), 'und', 'x-private'])
    assert.throws(() => jar._setOyaLocale(invalid), /[Nn]ative locale/);
  for (const invalid of [1, true, null, undefined, {}]) assert.throws(() => jar._setOyaLocale(invalid));
  jar._setOyaLocale(tag.toUpperCase());
  jar._setOyaLocale(tag);
  assert.throws(() => jar._setOyaLocale('ja-JP'), /cannot be changed/);
}
/** Locale-only policy survives cold worker restart without borrowing another session's timezone or CPU count. */
async function unconfiguredLocale(jar, window, url, windows) {
  const baseline = windowFor(session.fromPartition('locale-baseline'), []);
  windows.push(baseline);
  await baseline.loadURL(url);
  const host = await baseline.webContents.executeJavaScript('first');
  assert.deepEqual(await window.webContents.executeJavaScript('first'), host);
  assert.throws(() => jar._setOyaLocale('de-DE'), /before any session renderer/);
  const own = session.fromPartition('locale-only');
  configureLocale(own, 'tr-TR');
  const isolated = windowFor(own);
  windows.push(isolated);
  const expected = { ...host, ...localeExpected('tr-TR') };
  await check(isolated, url, expected);
  isolated.destroy();
  await own.serviceWorkers._stopAllWorkers();
  await own.serviceWorkers.startWorkerForScope(url);
  const resumed = windowFor(own);
  windows.push(resumed);
  await resumed.loadURL(url);
  assert.deepEqual(
    await resumed.webContents.executeJavaScript('fetch("/worker-first").then(reply=>reply.json())'),
    expected,
  );
  assert.deepEqual(await baseline.webContents.executeJavaScript('first'), host);
}
/** First-script policy must apply to cross-origin child frames without page-world shims. */
async function run() {
  await app.whenReady();
  const site = createServer((req, res) => {
    if (req.headers.host.startsWith('localhost')) {
      res.setHeader('Content-Type', 'text/html');
      return res.end(
        `<!doctype html><script>const first=${snapshot};onmessage=()=>parent.postMessage(first,'*')</script>`,
      );
    }
    serve(req, res);
  });
  site.listen(0, '127.0.0.1');
  await once(site, 'listening');
  const url = `http://127.0.0.1:${site.address().port}/`;
  const windows = [];
  try {
    const zones = [
      ['UTC', 0, 0, 1, 'en-US'],
      ['Asia/Tokyo', -540, 9, 8, 'de-DE'],
      ['America/New_York', 300, 19, 256, 'tr-TR'],
    ];
    for (const [zone, offset, hour, cores, tag] of zones) {
      const jar = session.fromPartition('timezone-' + zone);
      assert.equal(typeof jar._setOyaTimeZone, 'function', 'patched native timezone API required');
      for (const invalid of ['', 'Not/AZone', 'UTC\0hidden', 'x'.repeat(129)])
        assert.throws(() => jar._setOyaTimeZone(invalid), /Invalid/);
      jar._setOyaTimeZone(zone);
      jar._setOyaTimeZone(zone);
      assert.throws(() => jar._setOyaTimeZone(zone === 'UTC' ? 'Asia/Tokyo' : 'UTC'), /cannot be changed/);
      if (hardware) configureHardware(jar, cores);
      if (locale) configureLocale(jar, tag);
      const window = windowFor(jar);
      windows.push(window);
      await check(window, url, { zone, offset, hour, ...(hardware ? { cores } : {}), ...localeExpected(tag) });
      if (hardware) {
        jar._setOyaHardwareConcurrency(cores);
        assert.throws(() => jar._setOyaHardwareConcurrency(cores === 1 ? 2 : 1), /cannot be changed/);
      }
      if (locale) {
        jar._setOyaLocale(tag.toUpperCase());
        assert.throws(() => jar._setOyaLocale('ja-JP'), /cannot be changed/);
      }
      jar._setOyaTimeZone(zone);
      assert.throws(() => jar._setOyaTimeZone('Europe/London'), /cannot be changed/);
    }
    for (let i = 0; i < windows.length; i++) {
      const [zone, offset, hour, cores, tag] = zones[i];
      assert.deepEqual(await windows[i].webContents.executeJavaScript('first'), {
        zone,
        offset,
        hour,
        ...(hardware ? { cores } : {}),
        ...localeExpected(tag),
      });
    }
    assert.equal(new Set(windows.map((window) => window.webContents.getOSProcessId())).size, zones.length);
    await windows[1].loadURL(url + '?replacement');
    assert.deepEqual(await windows[1].webContents.executeJavaScript('first'), {
      zone: 'Asia/Tokyo',
      offset: -540,
      hour: 9,
      ...(hardware ? { cores: 8 } : {}),
      ...localeExpected('de-DE'),
    });
    const late = session.fromPartition('timezone-late');
    const window = windowFor(late);
    windows.push(window);
    await window.loadURL(url);
    assert.equal(
      await window.webContents.executeJavaScript('first.zone'),
      Intl.DateTimeFormat().resolvedOptions().timeZone,
    );
    assert.throws(() => late._setOyaTimeZone('Asia/Tokyo'), /before any session renderer/);
    if (hardware) await unconfiguredHardware(late, window, url, windows);
    if (locale) await unconfiguredLocale(late, window, url, windows);
    console.log(
      (locale
        ? 'PASS native session locale, hardware and timezone'
        : hardware
          ? 'PASS native session hardware and timezone'
          : 'PASS native session timezone') +
        ': first scripts, native Date arithmetic, cross-origin frames, dedicated/shared/service workers, partition isolation and immutable lifecycle',
    );
  } finally {
    for (const window of windows) if (!window.isDestroyed()) window.destroy();
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
