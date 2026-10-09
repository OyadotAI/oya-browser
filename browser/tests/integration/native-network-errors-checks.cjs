/** Every advertised failure reason completes the original native request with the actual mapped net error. */
const assert = require('node:assert/strict');
/** Await native event delivery with a finite test deadline. */
async function event(a, predicate) {
  for (let i = 0; i < 500; i++) {
    const found = a.events.find(predicate);
    if (found) return found.params;
    await new Promise((r) => setTimeout(r, 10));
  }
  throw Error('Native failure event did not arrive');
}
module.exports = async function failureChecks(a, session, hits) {
  const reasons = {
    Failed: 'FAILED',
    Aborted: 'ABORTED',
    TimedOut: 'TIMED_OUT',
    AccessDenied: 'ACCESS_DENIED',
    ConnectionClosed: 'CONNECTION_CLOSED',
    ConnectionReset: 'CONNECTION_RESET',
    ConnectionRefused: 'CONNECTION_REFUSED',
    ConnectionAborted: 'CONNECTION_ABORTED',
    ConnectionFailed: 'CONNECTION_FAILED',
    NameNotResolved: 'NAME_NOT_RESOLVED',
    InternetDisconnected: 'INTERNET_DISCONNECTED',
    AddressUnreachable: 'ADDRESS_UNREACHABLE',
    BlockedByClient: 'BLOCKED_BY_CLIENT',
    BlockedByResponse: 'BLOCKED_BY_RESPONSE',
  };
  await a.call('Oya.enableNetwork', {}, session);
  await a.call('Fetch.enable', { patterns: [{ urlPattern: '*/filter/failure-*' }] }, session);
  for (const [reason, error] of Object.entries(reasons)) {
    const path = '/filter/failure-' + reason;
    const request = a.call(
      'Runtime.evaluate',
      {
        expression: `fetch(${JSON.stringify(path)}).then(()=>"unexpected",()=>"failed")`,
        returnByValue: true,
        awaitPromise: true,
      },
      session,
    );
    const pause = await event(a, (e) => e.method === 'Fetch.requestPaused' && e.params.request.url.endsWith(path));
    await assert.rejects(
      a.call('Fetch.failRequest', { requestId: pause.requestId, errorReason: 'invalid' }, session),
      /reason/,
    );
    await a.call('Fetch.failRequest', { requestId: pause.requestId, errorReason: reason }, session);
    assert.equal((await request).result.value, 'failed');
    const failed = await event(
      a,
      (e) => e.method === 'Oya.networkEvent' && e.params.phase === 'error' && e.params.url.endsWith(path),
    );
    assert.equal(failed.error, 'net::ERR_' + error);
    assert.equal(hits[path] || 0, 0, 'failed request must never reach the fixture server');
    await assert.rejects(a.call('Fetch.continueRequest', { requestId: pause.requestId }, session), /foreign/);
  }
  await a.call('Fetch.disable', {}, session);
  await a.call('Oya.disableNetwork', {}, session);
  console.log(
    'PASS: all 14 native failure reasons, exact net errors, no server delivery, invalid-reason preservation and one-shot continuations',
  );
};
