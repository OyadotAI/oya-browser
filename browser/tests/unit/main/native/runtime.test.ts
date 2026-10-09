/** Native runtime mapping preserves document, connection and serialization boundaries. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeRuntime } from '../../../../src/main/native/runtime.ts';
import { validateParams } from '../../../../src/main/native-front-door/validation.ts';
/** A native frame seam, never an inspector or page-script transport. */
function fixture() {
  const calls: any[] = [];
  let document = 'first';
  const frame = {
    detached: false,
    origin: 'https://example.test',
    async _runOyaRuntime(...args: any[]) {
      calls.push(args);
      return args[2] === 'context' ? { context: document } : { result: { type: 'number', value: 42 } };
    },
  };
  const page = { webContents: { isDestroyed: () => false, mainFrame: frame } } as any;
  return {
    calls,
    page,
    frame,
    runtime: new NativeRuntime(),
    replace: () => {
      document = 'second';
    },
  };
}
test('native evaluation uses the main-world capability and a stable private owner', async () => {
  const f = fixture();
  assert.deepEqual(await f.runtime.execute(f.page, 'target', 'evaluate', { expression: '6*7', returnByValue: true }), {
    result: { type: 'number', value: 42 },
  });
  const [context, operation] = f.calls;
  assert.equal(context[2], 'context');
  assert.equal(operation[0], context[0]);
  assert.equal(operation[1], 'first');
  assert.deepEqual(operation.slice(2), ['evaluate', { source: '6*7', byValue: true, await: false }]);
  f.runtime.dispose();
  assert.equal(f.calls.at(-1)[2], 'close');
  await assert.rejects(f.runtime.execute(f.page, 'target', 'evaluate', { expression: '1' }), /closed/);
});
test('connection namespaces never share native values and public context ids are checked first', async () => {
  const f = fixture(),
    other = new NativeRuntime();
  await f.runtime.execute(f.page, 'target', 'evaluate', { expression: '1' });
  await other.execute(f.page, 'target', 'evaluate', { expression: '1' });
  assert.notEqual(f.calls[0][0], f.calls[2][0]);
  const count = f.calls.filter((c) => c[2] === 'evaluate').length;
  await assert.rejects(
    f.runtime.execute(f.page, 'target', 'evaluate', { expression: '1', contextId: 999999 }),
    /foreign/,
  );
  assert.equal(f.calls.filter((c) => c[2] === 'evaluate').length, count);
  f.runtime.dispose();
  other.dispose();
});
test('unsupported engines never substitute isolated-world or main-page helper evaluation', async () => {
  const f = fixture();
  delete (f.frame as any)._runOyaRuntime;
  await assert.rejects(f.runtime.execute(f.page, 'target', 'evaluate', { expression: '1' }), /lacks native/);
});
test('runtime validation rejects unsupported semantics and ambiguous argument representations', () => {
  for (const [method, params] of [
    ['Runtime.evaluate', {}],
    ['Runtime.evaluate', { expression: '1', contextId: '2' }],
    ['Runtime.evaluate', { expression: '1', contextId: 1, uniqueContextId: 'other' }],
    ['Runtime.evaluate', { expression: '1', userGesture: true }],
    ['Runtime.evaluate', { expression: '1', timeout: 1 }],
    ['Runtime.getProperties', { objectId: 'x', ownProperties: 'false' }],
    ['Runtime.callFunctionOn', { functionDeclaration: 'function(){}', arguments: [{ value: 1, objectId: 'other' }] }],
    [
      'Runtime.callFunctionOn',
      { functionDeclaration: 'function(){}', arguments: [{ unserializableValue: 'process.exit()' }] },
    ],
  ] as const)
    assert.throws(() => validateParams({ id: 1, method, params }));
});

test('inherited inspection uses a distinct native opcode while explicit own-only stays compatible', async () => {
  const f = fixture();
  (f.frame as any)._runOyaRuntime = async (...args: any[]) => {
    f.calls.push(args);
    if (args[2] === 'context') return { context: 'first' };
    if (args[2] === 'evaluate') return { result: { type: 'object', objectId: 'owned' } };
    return { properties: [] };
  };
  await f.runtime.execute(f.page, 'target', 'evaluate', { expression: '({})' });
  for (const ownProperties of [true, false, undefined]) {
    const params = { objectId: 'owned', ...(ownProperties === undefined ? {} : { ownProperties }) };
    validateParams({ id: 1, method: 'Runtime.getProperties', params });
    await f.runtime.execute(f.page, 'target', 'inspect', params);
    assert.equal(f.calls.at(-1)[2], ownProperties === true ? 'inspect' : 'inspectChain');
  }
});
test('older engines reject inherited inspection without falling back to own-only properties', async () => {
  const f = fixture();
  (f.frame as any)._runOyaRuntime = async (...args: any[]) => {
    f.calls.push(args);
    if (args[2] === 'context') return { context: 'first' };
    if (args[2] === 'evaluate') return { result: { type: 'object', objectId: 'owned' } };
    return { error: 'Unsupported native runtime operation' };
  };
  await f.runtime.execute(f.page, 'target', 'evaluate', { expression: '({})' });
  await assert.rejects(f.runtime.execute(f.page, 'target', 'inspect', { objectId: 'owned' }), /Unsupported native/);
  assert.equal(f.calls.at(-1)[2], 'inspectChain');
  assert.ok(!f.calls.some((c) => c[2] === 'inspect'));
});
