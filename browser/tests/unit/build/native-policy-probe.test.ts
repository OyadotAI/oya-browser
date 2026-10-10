/** Packaging cannot accept an old engine or fabricated pre-script policy readback. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { preparePolicy } = createRequire(import.meta.url)('../../../build/native-policy-probe.cjs');

it('refuses engines missing native pre-script policy without fallback', () => {
  assert.throws(() => preparePolicy({}), /Missing native pre-script/);
});
it('requires both native page and worker policy sources to round-trip', () => {
  let policy: object;
  const session = {
    _setOyaPreScriptPolicy: (value: object) => {
      policy = value;
    },
    _getOyaSessionPolicy: () => ({ preScriptPolicyVersion: 1, preScriptPolicy: policy }),
  };
  assert.doesNotThrow(() => preparePolicy(session));
  session._getOyaSessionPolicy = () => ({ preScriptPolicyVersion: 1, preScriptPolicy: {} });
  assert.throws(() => preparePolicy(session), /readback failed/);
});
