/** External isolated-world commands terminate in native worlds with ownership, origin and lifecycle fencing. */
const assert = require('node:assert/strict');
/** Wait for genuine native lifecycle delivery, not synthetic protocol acknowledgements. */
async function until(read) {
  for (let i = 0; i < 100; i++) {
    const value = read();
    if (value) return value;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw Error('Isolated-world event did not arrive');
}
/** Verify page, connection, frame, handle, group and human ownership through the external adapter. */
module.exports = async function worlds(a, b, wc, control) {
  await a.call('Runtime.enable');
  const tree = (await a.call('Page.getFrameTree')).frameTree;
  const create = (client, name, frameId = tree.frame.id) =>
    client.call('Page.createIsolatedWorld', { frameId, worldName: name, grantUniveralAccess: false });
  const one = (await create(a, 'agent-world')).executionContextId;
  const sibling = (await create(a, 'sibling-world')).executionContextId;
  const other = (await create(b, 'agent-world')).executionContextId;
  assert.equal((await create(a, 'agent-world')).executionContextId, one);
  assert.notEqual(other, one);
  const event = await until(() =>
    a.events.find((e) => e.method === 'Runtime.executionContextCreated' && e.params.context.id === one),
  );
  assert.equal(event.params.context.name, 'agent-world');
  assert.equal(event.params.context.auxData.isDefault, false);
  assert.equal(event.params.context.auxData.type, 'isolated');
  assert.equal(event.params.context.auxData.frameId, tree.frame.id);
  const evaluate = (id, expression, extra = {}) => a.call('Runtime.evaluate', { contextId: id, expression, ...extra });
  await wc.mainFrame._executeJavaScriptInOyaWorld('globalThis.privateRecorderMarker=93');
  await a.call('Runtime.evaluate', { expression: 'window.pageOnlyMarker=17' });
  assert.equal((await evaluate(one, 'globalThis.agentOnlyMarker=42')).result.value, 42);
  for (const expression of [
    'typeof pageOnlyMarker',
    'typeof privateRecorderMarker',
    'typeof process',
    'typeof require',
  ])
    assert.equal((await evaluate(one, expression)).result.value, 'undefined');
  assert.equal((await evaluate(sibling, 'typeof agentOnlyMarker')).result.value, 'undefined');
  assert.equal((await a.call('Runtime.evaluate', { expression: 'typeof agentOnlyMarker' })).result.value, 'undefined');
  assert.equal(
    (await b.call('Runtime.evaluate', { contextId: other, expression: 'typeof agentOnlyMarker' })).result.value,
    'undefined',
  );
  await assert.rejects(b.call('Runtime.evaluate', { contextId: one, expression: '1' }), /foreign/);
  assert.ok(
    (await evaluate(one, 'frames[0].document')).exceptionDetails,
    'normal cross-origin policy remains enforced',
  );
  await assert.rejects(
    a.call('Page.createIsolatedWorld', { frameId: tree.frame.id, grantUniveralAccess: true }),
    /universal/,
  );
  await assert.rejects(create(a, 'foreign-frame', 'not-owned'), /foreign/);
  await evaluate(one, 'document.querySelector("input").value="native worlds"');
  assert.equal((await evaluate(sibling, 'document.querySelector("input").value')).result.value, 'native worlds');
  const first = (await evaluate(one, '({answer:42})', { objectGroup: 'world-group' })).result.objectId;
  const second = (await evaluate(sibling, '({answer:7})', { objectGroup: 'world-group' })).result.objectId;
  assert.equal(
    (
      await a.call('Runtime.callFunctionOn', {
        objectId: first,
        functionDeclaration: 'function(){return this.answer + agentOnlyMarker}',
      })
    ).result.value,
    84,
  );
  await assert.rejects(
    a.call('Runtime.callFunctionOn', {
      objectId: first,
      functionDeclaration: 'function(x){globalThis.shouldNotRun=true}',
      arguments: [{ objectId: second }],
    }),
    /foreign/,
  );
  assert.equal((await evaluate(one, 'typeof shouldNotRun')).result.value, 'undefined');
  assert.equal((await evaluate(one, 'Promise.resolve(42)', { awaitPromise: true })).result.value, 42);
  await a.call('Runtime.releaseObjectGroup', { objectGroup: 'world-group' });
  for (const objectId of [first, second])
    await assert.rejects(a.call('Runtime.getProperties', { objectId }), /foreign/);
  const child = (await create(a, 'agent-world', tree.childFrames[0].frame.id)).executionContextId;
  assert.equal((await evaluate(child, 'typeof agentOnlyMarker')).result.value, 'undefined');
  const count = a.events.length;
  control.localHeld = true;
  try {
    await assert.rejects(create(a, 'human-blocked'), /human/);
  } finally {
    control.localHeld = false;
  }
  assert.equal(
    a.events.slice(count).some((e) => e.params?.context?.name === 'human-blocked'),
    false,
  );
  await a.call('Runtime.disable');
  await a.call('Runtime.enable');
  await until(
    () =>
      a.events.filter((e) => e.method === 'Runtime.executionContextCreated' && e.params.context.id === one).length ===
      2,
  );
  await a.call('Runtime.evaluate', { expression: 'document.querySelector("iframe").remove()' });
  await until(() =>
    a.events.some((e) => e.method === 'Runtime.executionContextDestroyed' && e.params.executionContextId === child),
  );
  await assert.rejects(evaluate(child, '1'), /foreign|closed/);
  assert.equal((await evaluate(one, 'agentOnlyMarker')).result.value, 42);
  await wc.reload();
  await new Promise((resolve) => wc.once('did-finish-load', resolve));
  await assert.rejects(evaluate(one, '1'), /foreign|closed/);
  const nextTree = (await a.call('Page.getFrameTree')).frameTree;
  const next = (await create(a, 'agent-world', nextTree.frame.id)).executionContextId;
  assert.notEqual(next, one);
  assert.equal((await evaluate(next, 'typeof agentOnlyMarker')).result.value, 'undefined');
  await a.call('Runtime.disable');
  console.log(
    'PASS: native isolated-world protocol, events, origin checks, agent/frame/handle isolation, human ownership, groups and navigation',
  );
};
