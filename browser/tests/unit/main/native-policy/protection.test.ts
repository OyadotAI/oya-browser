/** Full native protection cannot be bypassed by partial hooks, late configuration or policy mutation. */
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { NativeSessionProtection, nativePolicyForPersona } from '../../../../src/main/native-policy/index.ts';
import { nativeProtectionFixture } from '../../support/native-protection.ts';
/** A complete stable persona policy with the actual test engine version. */
function policy() {
  return nativePolicyForPersona(
    {
      navigator: { platform: 'Win32', hardwareConcurrency: 8, languages: ['en-US'] },
      locale: 'en-US',
      timezone: 'UTC',
    },
    'Chrome/152.0.7977.130',
  );
}
const scripts = { page: '(() => {})()', worker: '(() => {})()' };
test('complete installation is immutable and reusable after renderer startup', () => {
  const owner = new NativeSessionProtection(),
    { session, state, calls } = nativeProtectionFixture();
  owner.configure(session, policy(), scripts);
  state.rendererStarted = true;
  owner.configure(session, policy(), scripts);
  owner.assertConfigured(session);
  assert.equal(calls.filter((name) => name === 'scripts').length, 1);
  assert.throws(() => owner.configure(session, { ...policy(), timeZone: 'Europe/Paris' }, scripts), /policy changed/);
  assert.throws(() => owner.configure(session, policy(), { ...scripts, worker: '' }), /policy changed/);
});
test('already running sessions are refused without any native mutation', () => {
  const owner = new NativeSessionProtection(),
    { session, state, calls } = nativeProtectionFixture();
  state.rendererStarted = true;
  assert.throws(() => owner.configure(session, policy(), scripts), /cold session/);
  assert.deepEqual(calls, []);
});
test('source readback mismatch permanently quarantines the partial installation', () => {
  const owner = new NativeSessionProtection(),
    { session } = nativeProtectionFixture();
  session._setOyaPreScriptPolicy = () => {};
  assert.throws(() => owner.configure(session, policy(), scripts), /readback mismatch/);
  assert.throws(() => owner.assertConfigured(session), /not been installed/);
  assert.throws(() => owner.configure(session, policy(), scripts), /restart Oya/);
});
test('an unsupported source contract is refused before scalar setters', () => {
  const owner = new NativeSessionProtection(),
    { session, state, calls } = nativeProtectionFixture();
  state.preScriptPolicyVersion = 2;
  assert.throws(() => owner.configure(session, policy(), scripts), /Unsupported native/);
  assert.deepEqual(calls, []);
});
test('caller source mutations never change the installed engine snapshot', () => {
  const owner = new NativeSessionProtection(),
    { session, state } = nativeProtectionFixture();
  const requested = { ...scripts };
  owner.configure(session, policy(), requested);
  requested.page = 'changed';
  assert.deepEqual(state.preScriptPolicy, scripts);
});
