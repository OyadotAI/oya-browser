/** Packaging probe uses only native APIs in an isolated disposable profile, never a debugger endpoint. */
const { app, BrowserWindow } = require('electron');
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
/** Emit a strict architecture marker only after real native execution succeeds. */
async function run() {
  await app.whenReady();
  const window = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true } });
  try {
    await inspect(window);
    console.log('OYA_NATIVE_PROBE ' + process.platform + ' ' + process.arch);
  } finally { window.destroy(); }
}
/** Parent removes the disposable profile only after the engine has exited. */
function finish(code) {
  app.exit(code);
}
run().then(() => finish(0), (error) => { console.error(error.message); finish(1); });
