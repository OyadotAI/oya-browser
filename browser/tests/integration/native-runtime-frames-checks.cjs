/** Real cross-process native contexts and values, selected only through Oya's external front door. */
const assert = require('node:assert/strict');
/** Wait for native lifecycle delivery, never infer a context from its URL. */
async function until(read) {
  for (let i = 0; i < 500; i++) {
    const result = read();
    if (result) return result;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw Error('Native child-frame lifecycle did not arrive');
}
/** Contexts and opaque values must remain tied to one exact native document. */
module.exports = async function frames(a, b, wc) {
  a.events.length = 0;
  await a.call('Runtime.enable');
  const tree = (await a.call('Page.getFrameTree')).frameTree;
  assert.equal(tree.childFrames.length, 2);
  const contextFor = (id) =>
    a.events.find((e) => e.method === 'Runtime.executionContextCreated' && e.params.context.auxData.frameId === id)
      ?.params.context;
  const first = await until(() => contextFor(tree.childFrames[0].frame.id));
  const sibling = await until(() => contextFor(tree.childFrames[1].frame.id));
  assert.notEqual(first.id, sibling.id, 'identical URLs still have separate native contexts');
  const evaluate = (context, expression, extra = {}) =>
    a.call('Runtime.evaluate', { contextId: context.id, expression, ...extra });
  assert.equal((await evaluate(first, 'window===top')).result.value, false);
  await evaluate(first, 'window.nativeChildMarker="first"');
  await evaluate(sibling, 'window.nativeChildMarker="sibling"');
  const one = (await evaluate(first, '({name:nativeChildMarker})', { objectGroup: 'children' })).result.objectId;
  const two = (await evaluate(sibling, '({name:nativeChildMarker})', { objectGroup: 'children' })).result.objectId;
  assert.equal(
    (await a.call('Runtime.callFunctionOn', { objectId: one, functionDeclaration: 'function(){return this.name}' }))
      .result.value,
    'first',
  );
  const properties = (await a.call('Runtime.getProperties', { objectId: two, ownProperties: true })).result;
  assert.equal(properties.find((p) => p.name === 'name').value.value, 'sibling');
  await assert.rejects(
    a.call('Runtime.callFunctionOn', {
      objectId: one,
      functionDeclaration: 'function(x){window.crossFrameExecuted=true;return x}',
      arguments: [{ objectId: two }],
    }),
    /foreign.*context/,
  );
  assert.equal((await evaluate(first, 'typeof crossFrameExecuted')).result.value, 'undefined');
  await assert.rejects(b.call('Runtime.evaluate', { contextId: first.id, expression: '1' }), /foreign/);
  await assert.rejects(b.call('Runtime.getProperties', { objectId: one, ownProperties: true }), /foreign/);
  const promise = (await evaluate(first, 'Promise.resolve("child-promise")')).result.objectId;
  assert.equal((await a.call('Runtime.awaitPromise', { promiseObjectId: promise })).result.value, 'child-promise');
  await a.call('Runtime.releaseObjectGroup', { objectGroup: 'children' });
  for (const objectId of [one, two])
    await assert.rejects(a.call('Runtime.getProperties', { objectId, ownProperties: true }), /foreign/);
  const survivor = (await evaluate(sibling, '({name:nativeChildMarker})')).result.objectId;
  const destroyed = (id) =>
    a.events.some((e) => e.method === 'Runtime.executionContextDestroyed' && e.params.executionContextId === id);
  await a.call('Runtime.evaluate', {
    expression: `document.querySelector('iframe').src=${JSON.stringify(new URL('/child?replacement', wc.getURL()).href)}`,
  });
  await until(() => destroyed(first.id));
  const nextTree = (await a.call('Page.getFrameTree')).frameTree;
  const replacement = await until(
    () =>
      a.events.find(
        (e) =>
          e.method === 'Runtime.executionContextCreated' &&
          e.params.context.auxData.frameId === nextTree.childFrames[0].frame.id &&
          e.params.context.id !== first.id,
      )?.params.context,
  );
  await assert.rejects(evaluate(first, '1'), /foreign|closed/);
  await assert.rejects(a.call('Runtime.awaitPromise', { promiseObjectId: promise }), /foreign|closed/);
  assert.equal((await evaluate(replacement, 'typeof nativeChildMarker')).result.value, 'undefined');
  assert.equal(
    (
      await a.call('Runtime.callFunctionOn', {
        objectId: survivor,
        functionDeclaration: 'function(){return this.name}',
      })
    ).result.value,
    'sibling',
  );
  await a.call('Runtime.evaluate', { expression: 'setTimeout(()=>document.querySelector("iframe").remove(),100)' });
  await assert.rejects(evaluate(replacement, 'new Promise(()=>{})', { awaitPromise: true }), /document|frame|context/i);
  await until(() => destroyed(replacement.id));
  assert.equal(destroyed(sibling.id), false, 'removing a child must not revoke a surviving sibling');
  assert.equal((await evaluate(sibling, 'nativeChildMarker')).result.value, 'sibling');
  await a.call('Runtime.disable');
  console.log(
    'PASS: native cross-process child Runtime contexts, shared frame-tree identities, exact handle routing, group release, foreign-context rejection, navigation and removal cancellation with sibling preservation',
  );
};
