/** Opaque sandbox origins must return unavailable for WebAuthn without crashing the browser process. */
const assert = require('node:assert/strict');
const http = require('node:http');
const { once } = require('node:events');
const { app, BrowserWindow } = require('electron');
if (!process.env.OYA_WEBAUTHN_PROFILE) throw Error('Use the isolated native launcher');
app.setPath('userData', process.env.OYA_WEBAUTHN_PROFILE);
/** Query both browser-owned availability entry points from a secure but opaque sandbox document. */
async function run() {
  await app.whenReady();
  const server = http.createServer((_req, res) => res.end('<!doctype html><title>WebAuthn native regression</title>'));
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const win = new BrowserWindow({ show: false, webPreferences: { sandbox: true, contextIsolation: true } });
  Object.defineProperty(win.webContents, 'debugger', {
    get() {
      throw Error('Internal CDP forbidden');
    },
  });
  try {
    await win.loadURL(`http://127.0.0.1:${server.address().port}/`);
    const result = await win.webContents.executeJavaScript(`new Promise((resolve) => {
      const frame = document.createElement('iframe');
      frame.sandbox = 'allow-scripts';
      frame.srcdoc = '<script>Promise.all([PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable(), PublicKeyCredential.isConditionalMediationAvailable(), PublicKeyCredential.getClientCapabilities()]).then(values => parent.postMessage({values, secure: isSecureContext}, "*")).catch(error => parent.postMessage({error: error.name}, "*"));<\\/script>';
      window.addEventListener('message', event => { if(event.source === frame.contentWindow) resolve({origin:event.origin,...event.data}); });
      document.body.append(frame);
    })`);
    assert.equal(result.origin, 'null');
    assert.equal(result.secure, true);
    assert.deepEqual(result.values.slice(0, 2), [false, false]);
    assert.equal(result.values[2].userVerifyingPlatformAuthenticator, false);
    assert.equal(result.values[2].conditionalGet, false);
    assert.equal(
      await win.webContents.executeJavaScript('typeof PublicKeyCredential.isConditionalMediationAvailable() .then'),
      'function',
    );
    assert.equal(
      await win.webContents.executeJavaScript('PublicKeyCredential.isConditionalMediationAvailable()'),
      true,
    );
    console.log(
      'PASS native opaque-origin WebAuthn false/false; normal secure-origin conditional availability preserved',
    );
  } finally {
    win.destroy();
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
}
run()
  .then(() => app.exit(0))
  .catch((error) => {
    console.error(error);
    app.exit(1);
  });
