/** Reject engines that crash when secure sandbox frames query WebAuthn availability. */
const http = require('node:http');
const assert = require('node:assert/strict');
/** The opaque child uses actual public APIs and proves its null origin and secure context. */
const QUERY = `new Promise(resolve => {
  const frame = document.createElement('iframe'); frame.sandbox = 'allow-scripts';
  frame.srcdoc = '<script>Promise.all([PublicKeyCredential.isUserVerifyingPlatformAuthenticatorAvailable(),PublicKeyCredential.isConditionalMediationAvailable(),PublicKeyCredential.getClientCapabilities()]).then(values=>parent.postMessage({values,secure:isSecureContext},"*")).catch(error=>parent.postMessage({error:error.name},"*"));<\\/script>';
  addEventListener('message', event => { if(event.source === frame.contentWindow) resolve({origin:event.origin,...event.data}); });
  document.body.append(frame);
})`;
/** Use a local trustworthy origin; no external network or browser debugging connection is involved. */
async function inspect(contents) {
  const server = http.createServer((_request, response) => response.end('<!doctype html><title>Native probe</title>'));
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    await contents.loadURL(`http://127.0.0.1:${server.address().port}/`);
    const result = await contents.executeJavaScript(QUERY);
    assert.equal(result.origin, 'null');
    assert.equal(result.secure, true);
    assert.deepEqual(result.values.slice(0, 2), [false, false]);
    assert.equal(result.values[2].userVerifyingPlatformAuthenticator, false);
    assert.equal(result.values[2].conditionalGet, false);
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
}
module.exports = { inspect };
