/** Real main-world V8 semantics, owned handles and context cancellation with debugger access forbidden. */
const assert = require('node:assert/strict');
/** Native context notifications are asynchronous, not page-scanning polling. */
async function until(read) {
  for (let i = 0; i < 100; i++) {
    if (read()) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw Error('Native runtime event did not arrive');
}
/** Verify actual main-world state and native metadata, never a command-accepted-only result. */
module.exports = async function runtime(a, b, wc, control) {
  await wc.executeJavaScript('window.nativeValue={answer:41}; window.secretGetterCalls=0;');
  await wc.mainFrame._executeJavaScriptInOyaWorld('globalThis.agentWorldSecret = 91');
  await a.call('Runtime.enable');
  await until(() => a.events.some((e) => e.method === 'Runtime.executionContextCreated'));
  const { frameTree } = await a.call('Page.getFrameTree');
  const context = a.events.find(
    (e) => e.method === 'Runtime.executionContextCreated' && e.params.context.auxData.frameId === frameTree.frame.id,
  ).params.context;
  assert.equal(context.auxData.isDefault, true);
  const value = await a.call('Runtime.evaluate', {
    expression: 'nativeValue.answer + 1',
    contextId: context.id,
    returnByValue: true,
  });
  assert.equal(value.result.value, 42);
  assert.equal((await a.call('Runtime.evaluate', { expression: 'typeof agentWorldSecret' })).result.value, 'undefined');
  assert.equal((await a.call('Runtime.evaluate', { expression: 'undefined' })).result.type, 'undefined');
  assert.deepEqual((await a.call('Runtime.evaluate', { expression: 'null' })).result, {
    type: 'object',
    subtype: 'null',
    value: null,
  });
  for (const expression of ['NaN', 'Infinity', '-Infinity', '-0', '12345678901234567890n'])
    assert.equal((await a.call('Runtime.evaluate', { expression })).result.unserializableValue, expression);
  assert.deepEqual(
    (
      await a.call('Runtime.evaluate', {
        expression: '({nested:[1,true,null,{text:"日本語"}]})',
        returnByValue: true,
      })
    ).result.value,
    { nested: [1, true, null, { text: '日本語' }] },
  );
  await assert.rejects(
    a.call('Runtime.evaluate', {
      expression: '(()=>{const x={};x.self=x;return x})()',
      returnByValue: true,
    }),
    /by value/,
  );
  const special = await a.call('Runtime.callFunctionOn', {
    functionDeclaration: 'function(a,b,c){return [a===12345678901234567890n,Object.is(b,-0),Number.isNaN(c)]}',
    arguments: [
      { unserializableValue: '12345678901234567890n' },
      { unserializableValue: '-0' },
      { unserializableValue: 'NaN' },
    ],
    returnByValue: true,
  });
  assert.deepEqual(special.result.value, [true, true, true]);
  const symbolObject = (
    await a.call('Runtime.evaluate', {
      expression: '({[Symbol("private")]:42})',
      objectGroup: 'symbols',
    })
  ).result.objectId;
  const symbolProperty = (await a.call('Runtime.getProperties', { objectId: symbolObject, ownProperties: true }))
    .result[0];
  assert.equal(symbolProperty.name, 'Symbol(private)');
  assert.equal(symbolProperty.symbol.type, 'symbol');
  assert.equal(symbolProperty.value.value, 42);
  await a.call('Runtime.releaseObjectGroup', { objectGroup: 'symbols' });
  const object = (
    await a.call('Runtime.evaluate', {
      expression: '({answer:41, get danger(){secretGetterCalls++;throw Error("getter invoked")}})',
      objectGroup: 'owned',
    })
  ).result.objectId;
  assert.ok(object);
  const properties = (await a.call('Runtime.getProperties', { objectId: object, ownProperties: true })).result;
  assert.equal(properties.find((p) => p.name === 'answer').value.value, 41);
  assert.equal(properties.find((p) => p.name === 'danger').get.type, 'function');
  await wc.executeJavaScript(
    'Object.defineProperty(Object.prototype,"writable",{get(){secretGetterCalls++;throw Error("inherited getter")},configurable:true})',
  );
  try {
    const accessors = (
      await a.call('Runtime.getProperties', { objectId: object, ownProperties: true, accessorPropertiesOnly: true })
    ).result;
    assert.deepEqual(
      accessors.map((p) => p.name),
      ['danger'],
    );
  } finally {
    await wc.executeJavaScript('delete Object.prototype.writable');
  }
  assert.equal(await wc.executeJavaScript('secretGetterCalls'), 0);
  await assert.rejects(b.call('Runtime.getProperties', { objectId: object, ownProperties: true }), /foreign/);
  await assert.rejects(b.call('Runtime.evaluate', { expression: '1', contextId: context.id }), /foreign/);
  const call = await a.call('Runtime.callFunctionOn', {
    objectId: object,
    functionDeclaration: 'function(n){return this.answer+n}',
    arguments: [{ value: 1 }],
    returnByValue: true,
  });
  assert.equal(call.result.value, 42);
  const derived = (
    await a.call('Runtime.callFunctionOn', {
      objectId: object,
      functionDeclaration: 'function(){return {nested:this.answer}}',
    })
  ).result.objectId;
  assert.equal(
    (await a.call('Runtime.evaluate', { expression: 'Promise.resolve(42)', awaitPromise: true })).result.value,
    42,
  );
  const promise = (await a.call('Runtime.evaluate', { expression: 'Promise.resolve("awaited")' })).result.objectId;
  assert.equal(
    (await a.call('Runtime.awaitPromise', { promiseObjectId: promise, returnByValue: true })).result.value,
    'awaited',
  );
  assert.ok((await a.call('Runtime.evaluate', { expression: 'throw new Error("page-thrown")' })).exceptionDetails);
  assert.ok(
    (await a.call('Runtime.evaluate', { expression: 'Promise.reject("rejected")', awaitPromise: true }))
      .exceptionDetails,
  );
  const thrown = await a.call('Runtime.callFunctionOn', {
    objectId: object,
    functionDeclaration: 'function(){throw "call-thrown"}',
  });
  assert.equal(thrown.exceptionDetails.exception.value, 'call-thrown');
  await a.call('Runtime.releaseObjectGroup', { objectGroup: 'owned' });
  await assert.rejects(a.call('Runtime.getProperties', { objectId: object, ownProperties: true }), /foreign/);
  await assert.rejects(a.call('Runtime.getProperties', { objectId: derived, ownProperties: true }), /foreign/);
  await a.call('Runtime.releaseObject', { objectId: promise });
  await assert.rejects(a.call('Runtime.awaitPromise', { promiseObjectId: promise }), /foreign/);
  const abandoned = a.call('Runtime.evaluate', { expression: 'new Promise(()=>{})', awaitPromise: true });
  const cancelled = assert.rejects(abandoned, /context|document|frame/i);
  await new Promise((resolve) => setTimeout(resolve, 50));
  await wc.loadURL(wc.getURL());
  await cancelled;
  assert.equal((await a.call('Runtime.evaluate', { expression: '1' })).result.value, 1);
  await until(() => a.events.filter((e) => e.method === 'Runtime.executionContextCreated').length >= 2);
  await assert.rejects(a.call('Runtime.evaluate', { expression: '1', uniqueContextId: context.uniqueId }), /foreign/);
  const count = a.events.filter((e) => e.method.startsWith('Runtime.')).length;
  control.localHeld = true;
  await wc.loadURL(wc.getURL());
  control.localHeld = false;
  await a.call('Runtime.evaluate', { expression: '1' });
  assert.equal(
    a.events.filter((e) => e.method.startsWith('Runtime.')).length,
    count,
    'human contexts must not be replayed',
  );
  await a.call('Runtime.disable');
  console.log(
    'PASS: native main-world values, nonfinite primitives, handles, accessor-safe properties, function calls, promises, groups, navigation cancellation and human/context isolation',
  );
};
