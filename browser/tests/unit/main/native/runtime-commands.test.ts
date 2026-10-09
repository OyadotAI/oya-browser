/** Native runtime translation preserves value representations without forwarding protocol command names. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { runtimeParameters, runtimeOperation } from '../../../../src/main/native/runtime-commands.ts';
test('runtime invocation preserves explicit JSON, handle, nonfinite and undefined argument alternatives', () => {
  assert.deepEqual(
    runtimeParameters('invoke', {
      functionDeclaration: 'function(){}',
      objectId: 'receiver',
      objectGroup: 'group',
      returnByValue: true,
      awaitPromise: true,
      arguments: [
        { value: null },
        { value: false },
        { value: 0 },
        { objectId: 'argument' },
        { unserializableValue: '-0' },
        {},
      ],
    }),
    {
      source: 'function(){}',
      receiver: 'receiver',
      group: 'group',
      byValue: true,
      await: true,
      arguments: [{ value: null }, { value: false }, { value: 0 }, { objectId: 'argument' }, { special: '-0' }, {}],
    },
  );
});
test('native lifetime commands preserve their exact object or group selector', () => {
  assert.deepEqual(runtimeParameters('drop', { objectId: 'a' }), { object: 'a' });
  assert.deepEqual(runtimeParameters('dropGroup', { objectGroup: 'g' }), { group: 'g' });
  assert.deepEqual(runtimeParameters('await', { promiseObjectId: 'promise' }), {
    object: 'promise',
    byValue: false,
    await: true,
  });
  assert.deepEqual(runtimeParameters('inspect', { objectId: 'a', accessorPropertiesOnly: true }), {
    object: 'a',
    accessorsOnly: true,
  });
  assert.deepEqual(runtimeParameters('invoke', { functionDeclaration: 'function(){}' }), {
    source: 'function(){}',
    byValue: false,
    await: false,
    arguments: [],
  });
});
test('runtime operation selection never accepts an unknown opcode or downgrades inherited inspection', () => {
  assert.equal(runtimeOperation('inspect', {}), 'inspectChain');
  assert.equal(runtimeOperation('inspect', { ownProperties: false }), 'inspectChain');
  assert.equal(runtimeOperation('inspect', { ownProperties: true }), 'inspect');
  assert.equal(runtimeOperation('evaluate', {}), 'evaluate');
  assert.throws(() => runtimeParameters('toString', {}), /Unsupported/);
  assert.throws(() => runtimeParameters('Debugger.enable', {}), /Unsupported/);
});
