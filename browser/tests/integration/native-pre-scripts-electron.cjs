/** Actual unchanged persona sources must protect first scripts across native documents and every worker lifecycle. */
const { app, BrowserWindow, session } = require('electron');
const assert = require('node:assert/strict');
const { createServer } = require('node:http');
const { once } = require('node:events');
const { generateProfile } = require('../../anonymity/fingerprint.js');
const { buildInjectionScript, buildWorkerScript } = require('../../anonymity/inject.js');
const profile = process.env.OYA_PRE_SCRIPT_PROFILE;
if (!profile) throw Error('Use native-pre-scripts.mjs for parent-owned profile cleanup');
app.setPath('userData', profile);
app.commandLine.appendSwitch('site-per-process');
app.on('window-all-closed', () => {});
const windows = [];
const timeout = setTimeout(() => {
  console.error('Native pre-script fixture timed out');
  app.exit(1);
}, 120000);
const snapshot = `({runs:globalThis.__oyaFixtureRuns||0,platform:navigator.platform,hardware:navigator.hardwareConcurrency,memory:navigator.deviceMemory,language:navigator.language,languages:[...navigator.languages],zone:Intl.DateTimeFormat().resolvedOptions().timeZone})`;
/** Every route captures its identity at the first author statement. */
function serve(req, res) {
  res.setHeader('Content-Type', req.url.includes('.js') ? 'application/javascript' : 'text/html');
  const first = `const first=${snapshot};`;
  if (req.url.startsWith('/dedicated')) return res.end(first + 'postMessage(first);');
  if (req.url.startsWith('/shared')) return res.end(first + 'onconnect=e=>e.ports[0].postMessage(first);');
  if (req.url.startsWith('/sw'))
    return res.end(
      first +
        `oninstall=()=>skipWaiting();onactivate=e=>e.waitUntil(clients.claim());onmessage=e=>e.ports[0].postMessage(first);onfetch=e=>{if(new URL(e.request.url).pathname==='/worker-first')e.respondWith(Response.json(first))};`,
    );
  res.end(
    `<!doctype html><script>globalThis.first=${snapshot};onmessage=e=>e.source.postMessage(first,e.origin);</script><body>native fixture</body>`,
  );
}
/** All fixture surfaces use native APIs and fail immediately on debugger access. */
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
/** Actual production source remains byte-for-byte intact; only the fixture adds a once-per-context witness. */
function configure(jar, platform, seed) {
  const persona = generateProfile({ seed, platform, locale: 'en-US', timezone: 'America/New_York' });
  const witness = '\nglobalThis.__oyaFixtureRuns=(globalThis.__oyaFixtureRuns||0)+1;';
  const sources = { page: buildInjectionScript(persona) + witness, worker: buildWorkerScript(persona) + witness };
  assert.equal(jar._getOyaSessionPolicy().preScriptPolicyVersion, 1);
  for (const invalid of [
    { page: '', worker: '1' },
    { page: '(', worker: '1' },
    { page: '1', worker: '1', extra: true },
  ])
    assert.throws(() => jar._setOyaPreScriptPolicy(invalid));
  assert.equal(jar._getOyaSessionPolicy().preScriptPolicy, undefined);
  jar._setOyaTimeZone(persona.timezone);
  jar._setOyaHardwareConcurrency(persona.navigator.hardwareConcurrency);
  jar._setOyaLocale(persona.locale);
  jar._setOyaPlatform(platform);
  jar._setOyaPreScriptPolicy(sources);
  jar._setOyaPreScriptPolicy(sources);
  assert.deepEqual(jar._getOyaSessionPolicy().preScriptPolicy, sources);
  assert.throws(() => jar._setOyaPreScriptPolicy({ ...sources, worker: 'void 0' }), /cannot change/);
  return { persona, sources };
}
/** First author script on each worker and cross-process frame must see the session's installed persona. */
async function snapshots(window, url) {
  await window.loadURL(url);
  const read = (source) =>
    window.webContents.executeJavaScript(
      `Promise.resolve((${source})).catch(error=>{throw new Error(String(error?.stack||error))})`,
    );
  const values = { page: await read('first') };
  console.log('[pre-script] page first script', values.page.platform);
  for (const type of ['classic', 'module']) {
    values['dedicated-' + type] = await read(
      `new Promise((resolve,reject)=>{const w=new Worker('/dedicated.js?${type}',{type:'${type}'});w.onmessage=e=>{w.terminate();resolve(e.data)};w.onerror=e=>reject(new Error(e.message||"Worker startup failed"))})`,
    );
    console.log('[pre-script] dedicated', type);
    values['shared-' + type] = await read(
      `new Promise((resolve,reject)=>{const w=new SharedWorker('/shared.js?${type}',{type:'${type}'});w.port.onmessage=e=>{w.port.close();resolve(e.data)};w.onerror=e=>reject(new Error(e.message||"Worker startup failed"))})`,
    );
  }
  console.log('[pre-script] shared workers complete');
  values.service = await read(
    `(async()=>{await navigator.serviceWorker.register('/sw.js');const r=await navigator.serviceWorker.ready;return new Promise(resolve=>{const c=new MessageChannel();c.port1.onmessage=e=>{c.port1.close();resolve(e.data)};r.active.postMessage('read',[c.port2])})})()`,
  );
  console.log('[pre-script] classic service worker complete');
  values.serviceModule = await read(
    `(async()=>{const r=await navigator.serviceWorker.register('/sw-module.js',{type:'module',scope:'/module/'});const w=r.installing||r.waiting||r.active;if(w.state!=='activated')await new Promise(resolve=>w.addEventListener('statechange',()=>{if(w.state==='activated')resolve()}));return new Promise(resolve=>{const c=new MessageChannel();c.port1.onmessage=e=>{c.port1.close();resolve(e.data)};r.active.postMessage('read',[c.port2])})})()`,
  );
  const child = url.replace('127.0.0.1', 'localhost');
  values.frame = await read(
    `new Promise(resolve=>{const f=document.createElement('iframe');f.src=${JSON.stringify(child)};addEventListener('message',e=>{if(e.source===f.contentWindow)resolve(e.data)},{once:true});f.onload=()=>f.contentWindow.postMessage('read','*');document.body.append(f)})`,
  );
  return values;
}
/** Every captured field is checked; the witness also detects duplicate bootstrap execution. */
function expected(persona) {
  return {
    runs: 1,
    platform: persona.navigator.platform,
    hardware: persona.navigator.hardwareConcurrency,
    memory: persona.navigator.deviceMemory,
    language: persona.locale,
    languages: persona.navigator.languages,
    zone: persona.timezone,
  };
}
/** Native policy applies to popups before their first author statement as well. */
async function popup(window, url, result) {
  const opened = once(window.webContents, 'did-create-window');
  await window.webContents.executeJavaScript(`void window.open(${JSON.stringify(url)},'_blank')`);
  const [child] = await opened;
  windows.push(child);
  Object.defineProperty(child.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  if (child.webContents.isLoading()) await once(child.webContents, 'did-finish-load');
  assert.deepEqual(await child.webContents.executeJavaScript('first'), result, 'popup first script');
  child.destroy();
}
/** No-persona startup keeps an explicit empty worker policy while retaining page stealth. */
async function noPersonaStartup(url) {
  const jar = session.fromPartition('prescript-no-persona');
  const source = buildInjectionScript(null) + '\nglobalThis.__oyaFixtureRuns=1;';
  jar._setOyaPreScriptPolicy({ page: source, worker: '' });
  const window = windowFor(jar);
  await window.loadURL(url);
  assert.equal(await window.webContents.executeJavaScript('first.runs'), 1);
  assert.equal(jar._getOyaSessionPolicy().preScriptPolicy.worker, '');
  assert.equal(
    await window.webContents.executeJavaScript(
      `new Promise(resolve=>{const w=new Worker('/dedicated.js');w.onmessage=e=>{w.terminate();resolve(e.data.runs)}})`,
    ),
    0,
  );
  window.destroy();
}
/** Runtime source failures terminate the renderer before the first page statement can execute. */
async function rejectsPartialRuntime(url) {
  const jar = session.fromPartition('prescript-runtime-failure');
  jar._setOyaPreScriptPolicy({ page: 'throw new Error("fixture protection failed")', worker: 'void 0' });
  const window = windowFor(jar);
  const gone = once(window.webContents, 'render-process-gone');
  void window.loadURL(url).catch(() => {});
  const [, details] = await gone;
  assert.equal(details.reason, 'crashed', 'partial pre-script policy must not release an unprotected renderer');
  window.destroy();
}
/** Parallel partitions, no-policy host and cold service restart cannot share pre-script sources. */
async function run() {
  await app.whenReady();
  const site = createServer(serve);
  site.listen(0, '0.0.0.0');
  await once(site, 'listening');
  const url = `http://127.0.0.1:${site.address().port}/`;
  try {
    const host = session.fromPartition('prescript-host');
    const hostWindow = windowFor(host);
    await hostWindow.loadURL(url);
    assert.equal(await hostWindow.webContents.executeJavaScript('first.runs'), 0);
    assert.throws(() => host._setOyaPreScriptPolicy({ page: '1', worker: '1' }), /precede/);
    const configs = ['MacIntel', 'Win32'].map((platform, i) => {
      const jar = session.fromPartition('prescript-' + i);
      return { jar, ...configure(jar, platform, 'native-pre-script-' + i), window: windowFor(jar) };
    });
    await Promise.all(
      configs.map(async ({ jar, persona, sources, window }) => {
        for (const [surface, actual] of Object.entries(await snapshots(window, url)))
          assert.deepEqual(actual, expected(persona), surface);
        assert.equal(jar._getOyaSessionPolicy().rendererStarted, true);
        jar._setOyaPreScriptPolicy(sources);
        await popup(window, url + 'popup', expected(persona));
        await window.reload();
        if (window.webContents.isLoading()) await once(window.webContents, 'did-finish-load');
        assert.deepEqual(await window.webContents.executeJavaScript('first'), expected(persona), 'reload');
        for (const surface of windows)
          if (!surface.isDestroyed() && surface.webContents.session === jar) surface.destroy();
        await jar.serviceWorkers._stopAllWorkers();
        await jar.serviceWorkers.startWorkerForScope(url);
        await jar.serviceWorkers.startWorkerForScope(url + 'module/');
        const restored = windowFor(jar);
        await restored.loadURL(url);
        assert.deepEqual(
          await restored.webContents.executeJavaScript('fetch("/worker-first").then(r=>r.json())'),
          expected(persona),
          'cold service restart',
        );
        await restored.loadURL(url + 'module/client');
        assert.deepEqual(
          await restored.webContents.executeJavaScript('fetch("/worker-first").then(r=>r.json())'),
          expected(persona),
          'cold module service restart',
        );
      }),
    );
    assert.equal(await hostWindow.webContents.executeJavaScript('first.runs'), 0, 'parallel host partition untouched');
    await noPersonaStartup(url);
    await rejectsPartialRuntime(url);
    console.log(
      'PASS native pre-scripts: unchanged persona sources; first page/frame/popup and classic/module dedicated/shared/service workers; immutable policy; partition isolation; reload; cold service restart; no debugger',
    );
  } finally {
    for (const window of windows) if (!window.isDestroyed()) window.destroy();
    site.closeAllConnections();
    await new Promise((resolve) => site.close(resolve));
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
