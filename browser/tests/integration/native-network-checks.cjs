/** Verify native original-byte capture and paused request continuation through authenticated external Oya commands. */
const assert = require('node:assert/strict');
/** Wait only for native event delivery, never scan page text for a network state. */
async function until(read) {
  for (let i = 0; i < 500; i++) {
    const result = read();
    if (result) return result;
    await new Promise((r) => setTimeout(r, 10));
  }
  throw Error('Native network event did not arrive');
}
/** Actual byte equality and server counters distinguish native capture from a second request. */
module.exports = async function network(a, b, session, session2, deps, url, hits) {
  const evalPage = (expression, options = {}) =>
    a.call('Runtime.evaluate', { expression, returnByValue: true, ...options }, session);
  await a.call('Oya.enableNetwork', {}, session);
  const expected = Buffer.from([0, 255, 128, 17, 0, 65, 66, 67]);
  const result = await evalPage('fetch("/bytes").then(r=>r.arrayBuffer()).then(b=>Array.from(new Uint8Array(b)))', {
    awaitPromise: true,
  });
  assert.deepEqual(result.result.value, [...expected]);
  const request = await until(() =>
    a.events.find(
      (e) => e.method === 'Oya.networkEvent' && e.params.phase === 'request' && e.params.url.endsWith('/bytes'),
    ),
  );
  const body = await a.call('Network.getResponseBody', { requestId: request.params.requestId }, session);
  assert.equal(body.base64Encoded, true);
  assert.deepEqual(Buffer.from(body.body, 'base64'), expected);
  assert.equal(hits.bytes, 1);
  await a.call('Oya.enableNetwork', {}, session2);
  await assert.rejects(a.call('Network.getResponseBody', { requestId: request.params.requestId }, session2), /foreign/);
  await a.call('Oya.disableNetwork', {}, session2);
  const large = await evalPage('fetch("/large").then(r=>r.arrayBuffer()).then(b=>b.byteLength)', {
    awaitPromise: true,
  });
  assert.equal(large.result.value, 1024 * 1024);
  const overflow = await until(() =>
    a.events.find(
      (e) => e.method === 'Oya.networkEvent' && e.params.phase === 'request' && e.params.url.endsWith('/large'),
    ),
  );
  await assert.rejects(a.call('Network.getResponseBody', { requestId: overflow.params.requestId }, session), /limit/);
  assert.equal(hits.large, 1, 'overflow must not truncate or re-fetch the actual response');
  await a.call('Fetch.enable', { patterns: [{ requestStage: 'Request', urlPattern: '*' }] }, session);
  const running = evalPage('fetch("/paused").then(r=>r.text())', { awaitPromise: true });
  const paused = await until(() =>
    a.events.find((e) => e.method === 'Fetch.requestPaused' && e.params.request.url.endsWith('/paused')),
  );
  assert.equal(hits.paused, 0, 'native pause must occur before the server receives the request');
  await assert.rejects(a.call('Fetch.continueRequest', { requestId: paused.params.requestId }, session2), /foreign/);
  await a.call('Fetch.continueRequest', { requestId: paused.params.requestId }, session);
  assert.equal((await running).result.value, 'continued');
  assert.equal(hits.paused, 1);
  await assert.rejects(a.call('Fetch.continueRequest', { requestId: paused.params.requestId }, session), /foreign/);
  const blocked = evalPage('fetch("/blocked").then(()=>"unexpected",()=>"blocked")', { awaitPromise: true });
  const denied = await until(() =>
    a.events.find((e) => e.method === 'Fetch.requestPaused' && e.params.request.url.endsWith('/blocked')),
  );
  await a.call('Fetch.failRequest', { requestId: denied.params.requestId, errorReason: 'BlockedByClient' }, session);
  assert.equal((await blocked).result.value, 'blocked');
  assert.equal(hits.blocked, 0);
  await a.call('Fetch.disable', {}, session);
  await a.call('Oya.disableNetwork', {}, session);
  await assert.rejects(
    a.call('Network.getResponseBody', { requestId: request.params.requestId }, session),
    /foreign|revoked/,
  );
  console.log(
    'PASS: native binary response capture without refetch, nontruncating overflow, request-stage pause/continue/fail, held evaluation unblocking, exact target handles and observer revocation',
  );
};
