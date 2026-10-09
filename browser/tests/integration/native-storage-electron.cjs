/** Native first-party localStorage is restored before page scripts, observed without injection, and partition-isolated. */
const assert = require('node:assert/strict');
const http = require('node:http');
const { once } = require('node:events');
const { app, BrowserWindow, session } = require('electron');
if (!process.env.OYA_STORAGE_PROFILE) throw Error('Use the disposable native storage launcher');
app.setPath('userData', process.env.OYA_STORAGE_PROFILE);
app.on('window-all-closed', () => {});
/** Wait for an actual native observer event, with a bounded failure rather than a guessed delay. */
function mutation(jar, origin, write) {
  return new Promise((resolve, reject) => {
    const done = (error) => {
      clearTimeout(timer);
      jar.off('oya-local-storage-changed', changed);
      error ? reject(error) : resolve();
    };
    const changed = (_event, changedOrigin, available) => {
      if (changedOrigin === origin) done(available ? null : Error('Native storage observer disconnected'));
    };
    const timer = setTimeout(() => done(Error('Native storage mutation was not observed')), 5000);
    jar.on('oya-local-storage-changed', changed);
    Promise.resolve().then(write).catch(done);
  });
}
/** Every assertion uses real storage service state; debugger attachment is made fatal. */
async function run() {
  await app.whenReady();
  const site = http.createServer((_req, res) => {
    res.setHeader('Content-Type', 'text/html');
    res.end('<!doctype html><script>document.title=localStorage.getItem("account")||"Signed out"</script>');
  });
  site.listen(0, '127.0.0.1');
  await once(site, 'listening');
  const origin = `http://127.0.0.1:${site.address().port}`;
  const jar = session.fromPartition('native-storage-first');
  const other = session.fromPartition('native-storage-second');
  for (const name of [
    '_readOyaLocalStorage',
    '_restoreOyaLocalStorage',
    '_watchOyaLocalStorage',
    '_unwatchOyaLocalStorage',
  ])
    assert.equal(typeof jar[name], 'function', 'Missing patched native API: ' + name);
  const win = new BrowserWindow({
    show: false,
    webPreferences: { session: jar, sandbox: true, contextIsolation: true, nodeIntegration: false },
  });
  const wc = win.webContents;
  Object.defineProperty(wc, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  const read = async () => Object.fromEntries(await jar._readOyaLocalStorage(origin));
  const values = [
    ['account', 'Alice'],
    ['unicode', 'مرحبا 🗝'],
    ['nul\0key', '\0value'],
    ['surrogate', '\ud800'],
    ['__proto__', 'ordinary storage key'],
  ];
  try {
    await jar._restoreOyaLocalStorage(origin, values);
    assert.deepEqual(await read(), Object.fromEntries(values));
    assert.deepEqual(await other._readOyaLocalStorage(origin), []);
    await wc.loadURL(origin);
    assert.equal(wc.getTitle(), 'Alice', 'storage is present before the first page script');
    assert.equal(await wc.executeJavaScript('localStorage.getItem("unicode")'), 'مرحبا 🗝');
    assert.equal(await wc.executeJavaScript('localStorage.getItem("surrogate").charCodeAt(0)'), 0xd800);
    await jar._restoreOyaLocalStorage(origin, [['account', 'stale']]);
    assert.equal((await read()).account, 'Alice', 'existing local state wins over imported state');
    for (const invalid of ['file:///tmp', 'data:text/plain,x', origin + '/path', 'null'])
      await assert.rejects(jar._readOyaLocalStorage(invalid), /Invalid native localStorage/);
    await assert.rejects(
      jar._restoreOyaLocalStorage(origin, [
        ['dup', 'a'],
        ['dup', 'b'],
      ]),
      /Invalid native localStorage/,
    );
    await assert.rejects(
      jar._restoreOyaLocalStorage(origin, [['oversize', 'x'.repeat(1024 * 1024 + 1)]]),
      /Invalid native localStorage/,
    );
    assert.equal((await read()).account, 'Alice', 'invalid imports do not mutate the store');
    await jar._watchOyaLocalStorage(origin);
    await assert.rejects(jar._watchOyaLocalStorage('file:///tmp'), /Invalid/);
    await mutation(jar, origin, () => wc.executeJavaScript('localStorage.setItem("account", "Bob")'));
    assert.equal((await read()).account, 'Bob');
    await mutation(jar, origin, () => wc.executeJavaScript('localStorage.clear()'));
    assert.deepEqual(await read(), {});
    await wc.loadURL(origin);
    assert.equal(wc.getTitle(), 'Signed out', 'native reads do not resurrect cleared login state');
    jar._unwatchOyaLocalStorage(origin);
    const cancelledOrigin = `http://localhost:${site.address().port}`;
    const watching = jar._watchOyaLocalStorage(cancelledOrigin);
    jar._unwatchOyaLocalStorage(cancelledOrigin);
    await assert.rejects(watching, /watch closed/);
    assert.deepEqual(await other._readOyaLocalStorage(origin), []);
    console.log(
      'PASS: native storage hydration before scripts, Unicode/NUL/surrogates, isolation, validation, non-overwrite, mutation and clear events; debugger forbidden',
    );
  } finally {
    jar._unwatchOyaLocalStorage(origin);
    win.destroy();
    site.closeAllConnections();
    await new Promise((resolve) => site.close(resolve));
  }
}
run().then(
  () => app.exit(0),
  (error) => {
    console.error(error);
    app.exit(1);
  },
);
