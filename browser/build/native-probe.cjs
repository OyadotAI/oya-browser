/** Packaging probe uses only native APIs in an isolated disposable profile, never a debugger endpoint. */
const { app, BrowserWindow, session } = require('electron');
/** Discard engine probe state rather than opening a user's signed-in browser profile. */
const profile = process.env.OYA_RELEASE_PROBE_PROFILE;
if (!profile) throw Error('Launch through the native distribution verifier to supply a disposable profile');
app.setPath('userData', profile);
/** Require actual native entry points instead of a renamed stock distribution. */
function capabilities(contents) {
  for (const name of ['_executeJavaScriptInOyaWorld', '_runOyaRuntime'])
    if (typeof contents.mainFrame[name] !== 'function') throw Error('Missing native frame API: ' + name);
  if (typeof contents._insertTextOya !== 'function') throw Error('Missing native text acknowledgement');
  const requests = contents.session.webRequest;
  if (!requests._supportsOyaRequestErrors?.() || !contents._supportsOyaFrameLifecycle?.())
    throw Error('Missing native network lifecycle/error support');
  if (typeof requests._setOyaBodyListener !== 'function') throw Error('Missing native response capture');
}
/** Execute in the engine-owned isolated world to prove the capability is callable, not merely present. */
async function inspect(window) {
  const contents = window.webContents;
  Object.defineProperty(contents, 'debugger', { get() { throw Error('CDP backend forbidden'); } });
  await contents.loadURL('about:blank');
  capabilities(contents);
  const result = await contents.mainFrame._executeJavaScriptInOyaWorld('6 * 7', false);
  if (result !== 42) throw Error('Native isolated execution failed');
}
/** Exercise the session-owned native storage APIs required by persona synchronization. */
async function inspectStorage() {
  const jar = session.fromPartition('native-engine-storage-probe');
  const origin = 'https://native-storage-probe.invalid';
  for (const name of ['_readOyaLocalStorage', '_restoreOyaLocalStorage', '_watchOyaLocalStorage', '_unwatchOyaLocalStorage'])
    if (typeof jar[name] !== 'function') throw Error('Missing native storage API: ' + name);
  await jar._restoreOyaLocalStorage(origin, [['probe', 'native']]);
  await jar._watchOyaLocalStorage(origin);
  try {
    const values = await jar._readOyaLocalStorage(origin);
    if (values.length !== 1 || values[0][0] !== 'probe' || values[0][1] !== 'native') throw Error('Native storage probe failed');
  } finally { jar._unwatchOyaLocalStorage(origin); }
}
/** Emit a strict architecture marker only after real native execution succeeds. */
async function run() {
  await app.whenReady();
  const window = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true } });
  try {
    await inspect(window);
    await inspectStorage();
    console.log('OYA_NATIVE_PROBE ' + process.platform + ' ' + process.arch);
  } finally { window.destroy(); }
}
/** Parent removes the disposable profile only after the engine has exited. */
function finish(code) {
  app.exit(code);
}
run().then(() => finish(0), (error) => { console.error(error.message); finish(1); });
