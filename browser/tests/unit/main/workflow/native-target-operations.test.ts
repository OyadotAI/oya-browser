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
    'armPointer',
    'pointerReached',
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
it('admits a target only after trusted movement and removes its listener', () => {
  const listeners = new Map<string, (event: Pick<MouseEvent, 'isTrusted'>) => void>();
  const node = {
    isConnected: true,
    addEventListener: (type: string, listener: (event: Pick<MouseEvent, 'isTrusted'>) => void) =>
      listeners.set(type, listener),
    removeEventListener: (type: string) => listeners.delete(type),
  };
  const target: any = { key: 'owned-document', token: 'owned-node' };
  const world = { 'owned-document': { node, token: target.token } };
  const execute = (operation: string) => new Function('globalThis', `return ${targetScript(target, operation)}`)(world);
  execute('armPointer');
  listeners.get('mousemove')!({ isTrusted: false });
  assert.equal(execute('pointerReached'), false);
  listeners.get('mousemove')!({ isTrusted: true });
  assert.equal(execute('pointerReached'), true);
  assert.equal(listeners.size, 0);
});
