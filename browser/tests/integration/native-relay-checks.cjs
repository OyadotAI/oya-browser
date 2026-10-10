/** Exercise the production control-socket compatibility relay against real native Oya renderer operations. */
const assert = require('node:assert/strict');
const { CdpRelay } = require('../../src/main/connection/cdp-relay.ts');

/** Route correlated messages through the same serialized envelope used by the authenticated server socket. */
function clients(backend, control) {
  const pending = new Map();
  let sequence = 0;
  const relay = new CdpRelay({
    backend,
    allowed: () => !control.localHeld,
    socket: {
      send(message) {
        if (message.type !== 'cdp') return;
        const frame = JSON.parse(message.data);
        const key = `${message.sid}:${frame.id}`;
        const task = pending.get(key);
        if (!task) return;
        pending.delete(key);
        if (frame.error) task.reject(Error(frame.error.message));
        else task.resolve(frame.result);
      },
    },
  });
  const call = (sid, method, params = {}, sessionId) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      pending.set(`${sid}:${id}`, { resolve, reject });
      relay.cdpRelays.get(sid).send(JSON.stringify({ id, method, params, sessionId }));
    });
  return { relay, call };
}

/** Native values, trusted input, handle isolation and human revocation must survive the production relay envelope. */
module.exports = async function relayChecks(backend, wc, control) {
  const { relay, call } = clients(backend, control);
  await relay.openCdpRelay('first');
  await relay.openCdpRelay('second');
  const targetId = backend.targets()[0].targetId;
  try {
    const first = await call('first', 'Target.attachToTarget', { targetId, flatten: true });
    const second = await call('second', 'Target.attachToTarget', { targetId, flatten: true });
    const a = (method, params) => call('first', method, params, first.sessionId);
    const b = (method, params) => call('second', method, params, second.sessionId);
    assert.equal((await a('Runtime.evaluate', { expression: '21*2', returnByValue: true })).result.value, 42);
    const object = (await a('Runtime.evaluate', { expression: '({relaySecret:42})' })).result.objectId;
    assert.ok(object);
    await assert.rejects(b('Runtime.getProperties', { objectId: object, ownProperties: true }), /foreign|unknown/i);
    await wc.executeJavaScript(
      'document.querySelector("input").value="";document.querySelector("input").focus();globalThis.relayTrusted=false;document.querySelector("input").addEventListener("input",e=>globalThis.relayTrusted=e.isTrusted)',
    );
    await a('Input.insertText', { text: 'native relay 日本語' });
    assert.equal(await wc.executeJavaScript('document.querySelector("input").value'), 'native relay 日本語');
    assert.equal(await wc.executeJavaScript('relayTrusted'), true);
    control.localHeld = true;
    await assert.rejects(a('Runtime.evaluate', { expression: 'globalThis.relayChanged=true' }), /unavailable|revoked/);
    await assert.rejects(a('Input.insertText', { text: 'denied' }), /unavailable|revoked/);
    assert.equal(await wc.executeJavaScript('typeof relayChanged'), 'undefined');
    assert.equal(await wc.executeJavaScript('document.querySelector("input").value'), 'native relay 日本語');
  } finally {
    control.localHeld = false;
    relay.closeCdpRelays();
    assert.equal(relay.cdpRelays.size, 0);
    await wc.loadURL(wc.getURL());
  }
  console.log(
    'PASS production native relay: real runtime, trusted input, separate client handles and human takeover; debugger forbidden',
  );
};
