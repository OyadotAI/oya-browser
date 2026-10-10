/** Native workflow chords preserve physical keys and refuse malformed native UI requests. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { workflowKey } from '../../../../src/main/workflow/native-keys.ts';
import { Modifier } from '../../../../src/main/input/constants.ts';
it('combines modifiers while preserving the last physical key', () => {
  assert.deepEqual(workflowKey('Control+Shift+A'), { key: 'A', modifiers: Modifier.CTRL | Modifier.SHIFT });
  assert.deepEqual(workflowKey('+'), { key: '+', modifiers: 0 });
  assert.deepEqual(workflowKey('ControlOrMeta+a'), {
    key: 'a',
    modifiers: process.platform === 'darwin' ? Modifier.META : Modifier.CTRL,
  });
});
it('refuses malformed chords and keys reserved for native browser UI', () => {
  for (const value of ['Super+a', 'Control+', 'F12', 'F11', 'badkey'])
    assert.throws(() => workflowKey(value), /Unsupported native workflow/);
});
