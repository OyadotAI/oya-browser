/** Bounded prototype descriptors use native V8 identity and never execute accessors or proxy traps. */
const assert = require('node:assert/strict');
module.exports = async function propertyChecks(a) {
  const evaluate = async (expression) =>
    (await a.call('Runtime.evaluate', { expression, objectGroup: 'prototype-qa' })).result;
  const root = await evaluate(`(()=>{
    globalThis.prototypeEffects=0;
    const symbol=Symbol('same'), other=Symbol('same');
    const proto=Object.create(null);
    Object.defineProperties(proto, {inherited:{value:42}, shadowed:{get(){prototypeEffects++;return 'wrong'}}, danger:{get(){prototypeEffects++;throw Error('getter called')}}});
    proto[symbol]='parent';proto[other]='other-symbol';
    const object=Object.create(proto);object[symbol]='child';
    Object.defineProperty(object,'shadowed',{value:'own'});
    return object;
  })()`);
  const all = (await a.call('Runtime.getProperties', { objectId: root.objectId })).result;
  assert.equal(all.find((p) => p.name === 'inherited').value.value, 42);
  assert.equal(all.find((p) => p.name === 'inherited').isOwn, false);
  assert.equal(all.find((p) => p.name === 'shadowed').value.value, 'own');
  assert.equal(all.filter((p) => p.name === 'shadowed').length, 1);
  assert.equal(all.find((p) => p.name === 'danger').get.type, 'function');
  assert.equal(all.filter((p) => p.symbol).length, 2, 'same-description symbols retain distinct identities');
  assert.deepEqual(
    all
      .filter((p) => p.symbol)
      .map((p) => p.value.value)
      .sort(),
    ['child', 'other-symbol'],
  );
  const accessors = (await a.call('Runtime.getProperties', { objectId: root.objectId, accessorPropertiesOnly: true }))
    .result;
  assert.deepEqual(
    accessors.map((p) => p.name),
    ['danger'],
  );
  const own = (await a.call('Runtime.getProperties', { objectId: root.objectId, ownProperties: true })).result;
  assert.ok(own.every((p) => p.isOwn));
  assert.equal(own.length, 2);
  assert.equal((await evaluate('prototypeEffects')).value, 0);
  const proxy = await evaluate(
    'Object.create(new Proxy({}, {ownKeys(){prototypeEffects++;return []},getPrototypeOf(){prototypeEffects++;return null}}))',
  );
  await assert.rejects(a.call('Runtime.getProperties', { objectId: proxy.objectId }), /proxy/);
  assert.equal((await evaluate('prototypeEffects')).value, 0);
  assert.deepEqual(
    (await a.call('Runtime.getProperties', { objectId: proxy.objectId, ownProperties: true })).result,
    [],
  );
  const deep = await evaluate('(()=>{let x=Object.create(null);for(let i=0;i<33;i++)x=Object.create(x);return x})()');
  await assert.rejects(a.call('Runtime.getProperties', { objectId: deep.objectId }), /depth limit/);
  const wide = await evaluate(
    '(()=>{const p=Object.create(null),x=Object.create(p);for(let i=0;i<300;i++){p["p"+i]=i;x["x"+i]=i}return x})()',
  );
  await assert.rejects(a.call('Runtime.getProperties', { objectId: wide.objectId }), /property limit/);
  await a.call('Runtime.releaseObjectGroup', { objectGroup: 'prototype-qa' });
  await assert.rejects(
    a.call('Runtime.callFunctionOn', {
      objectId: all.find((p) => p.name === 'danger').get.objectId,
      functionDeclaration: 'function(){return 1}',
    }),
    /foreign/,
  );
  console.log(
    'PASS: native inherited descriptors, shadowing, symbol identity, accessor-only filtering, uncalled getters/proxy traps, bounded prototype walks and inherited-handle group release',
  );
};
