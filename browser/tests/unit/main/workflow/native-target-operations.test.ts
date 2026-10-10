/** Every dispatched isolated node operation must parse independently and reject unknown operations. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { targetScript } from '../../../../src/main/workflow/native-target-operations.ts';
it('emits valid isolated functions for each supported action without lexical collisions', () => {
  const target: any = { key: 'owned-document', token: 'owned-node' };
  for (const operation of [
    'point',
    'editable',
    'focused',
    'valid',
    'select',
    'file',
    'upload',
    'fileNames',
    'fillValue',
  ]) {
    assert.doesNotThrow(() => new Function(targetScript(target, operation)));
  }
  assert.throws(() => targetScript(target, 'unknown'), /Unsupported native target operation/);
});
