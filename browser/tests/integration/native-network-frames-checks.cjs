/** Child request flow uses native frame attribution, original response bytes and exact-document cancellation. */
const assert = require('node:assert/strict');
/** Wait for native event delivery rather than guessing from page text. */
async function until(read) {
  for (let i = 0; i < 500; i++) {
    const value = read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw Error('Native child network event did not arrive');
}
/** Two cross-process siblings must not share network identities or cancellation state. */
module.exports = async function networkFrames(a, session, hits) {
  await a.call('Runtime.enable', {}, session);
  const tree = (await a.call('Page.getFrameTree', {}, session)).frameTree;
  assert.equal(tree.childFrames.length, 2);
  const contexts = await Promise.all(
    tree.childFrames.map(({ frame }) =>
      until(
        () =>
          a.events.find(
            (e) =>
              e.sessionId === session &&
              e.method === 'Runtime.executionContextCreated' &&
              e.params.context.auxData.frameId === frame.id,
          )?.params.context,
      ),
    ),
  );
  const evaluate = (index, expression, extra = {}) =>
    a.call('Runtime.evaluate', { contextId: contexts[index].id, expression, ...extra }, session);
  const pauseFor = (path) =>
    until(
      () => a.events.find((e) => e.method === 'Fetch.requestPaused' && e.params.request.url.endsWith(path))?.params,
    );
  await a.call('Oya.enableNetwork', {}, session);
  const before = hits.bytes;
  await evaluate(0, 'fetch("/bytes").then(r=>r.arrayBuffer()).then(b=>b.byteLength)', { awaitPromise: true });
  const observed = await until(
    () =>
      a.events.find(
        (e) =>
          e.method === 'Oya.networkEvent' &&
          e.params.phase === 'request' &&
          e.params.frameId === tree.childFrames[0].frame.id &&
          e.params.url.endsWith('/bytes'),
      )?.params,
  );
  const body = await a.call('Network.getResponseBody', { requestId: observed.requestId }, session);
  assert.deepEqual(Buffer.from(body.body, 'base64'), Buffer.from([0, 255, 128, 17, 0, 65, 66, 67]));
  assert.equal(hits.bytes, before + 1, 'child response capture must not refetch');
  await a.call('Fetch.enable', {}, session);
  const pending = evaluate(0, 'fetch("/child-continued").then(r=>r.text())', { awaitPromise: true });
  const paused = await pauseFor('/child-continued');
  assert.equal(paused.frameId, tree.childFrames[0].frame.id);
  assert.equal(hits['child-continued'] || 0, 0);
  await a.call('Fetch.continueRequest', { requestId: paused.requestId }, session);
  assert.equal((await pending).result.value, 'child-continued');
  await evaluate(0, 'window.pending=fetch("/child-cancelled").catch(()=>"cancelled");"started"');
  const cancelled = await pauseFor('/child-cancelled');
  await evaluate(1, 'window.pending=fetch("/child-sibling").then(r=>r.text());"started"');
  const sibling = await pauseFor('/child-sibling');
  assert.equal(sibling.frameId, tree.childFrames[1].frame.id);
  await a.call('Runtime.evaluate', { expression: 'document.querySelector("iframe").remove()' }, session);
  await until(() =>
    a.events.some(
      (e) => e.method === 'Runtime.executionContextDestroyed' && e.params.executionContextId === contexts[0].id,
    ),
  );
  await assert.rejects(
    a.call('Fetch.continueRequest', { requestId: cancelled.requestId }, session),
    /foreign|authorized/,
  );
  assert.equal(hits['child-cancelled'] || 0, 0, 'removed child never sends its held request');
  await a.call('Fetch.continueRequest', { requestId: sibling.requestId }, session);
  assert.equal((await evaluate(1, 'pending', { awaitPromise: true })).result.value, 'child-sibling');
  assert.equal(hits['child-sibling'], 1);
  await a.call('Fetch.disable', {}, session);
  await a.call('Oya.disableNetwork', {}, session);
  await a.call('Runtime.disable', {}, session);
  console.log(
    'PASS: native cross-process child network identity, original binary bytes without refetch, urgent continuation, removal cancellation and independent sibling continuation',
  );
};
