/** Native listener aliases retain mandatory authentication and refuse widened network exposure. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nativeDoorConfig } from '../../../../src/main/native-front-door/index.ts';
const token = 'a'.repeat(32);

test('legacy listener port selects authenticated native configuration', () => {
  assert.deepEqual(nativeDoorConfig({ OYA_REMOTE_DEBUGGING_PORT: '9222', OYA_NATIVE_CDP_TOKEN: token }), {
    port: 9222,
    token,
  });
  assert.equal(nativeDoorConfig({}), null);
});
test('legacy alias cannot activate an unauthenticated listener', () => {
  assert.throws(() => nativeDoorConfig({ OYA_REMOTE_DEBUGGING_PORT: '9222' }), /TOKEN/);
});
test('conflicting aliases fail instead of starting an unexpected listener', () => {
  assert.throws(
    () =>
      nativeDoorConfig({ OYA_REMOTE_DEBUGGING_PORT: '9222', OYA_NATIVE_CDP_PORT: '9333', OYA_NATIVE_CDP_TOKEN: token }),
    /Conflicting/,
  );
});
test('explicit wildcard and remote addresses fail instead of silently broadening access', () => {
  for (const host of ['0.0.0.0', '::', '192.0.2.1', 'localhost'])
    assert.throws(
      () =>
        nativeDoorConfig({
          OYA_REMOTE_DEBUGGING_PORT: '9222',
          OYA_REMOTE_DEBUGGING_HOST: host,
          OYA_NATIVE_CDP_TOKEN: token,
        }),
      /127.0.0.1/,
    );
});
test('native listener ports reject coercion and out-of-range values', () => {
  for (const port of ['-1', '65536', '1.2', ' ', '0x123', '1e3'])
    assert.throws(() => nativeDoorConfig({ OYA_NATIVE_CDP_PORT: port, OYA_NATIVE_CDP_TOKEN: token }), /port/);
});

test('legacy zero disables the listener while explicit native zero requests an ephemeral listener', () => {
  assert.equal(nativeDoorConfig({ OYA_REMOTE_DEBUGGING_PORT: '0' }), null);
  assert.deepEqual(
    nativeDoorConfig({ OYA_REMOTE_DEBUGGING_PORT: '0', OYA_NATIVE_CDP_PORT: '0', OYA_NATIVE_CDP_TOKEN: token }),
    { port: 0, token },
  );
});
