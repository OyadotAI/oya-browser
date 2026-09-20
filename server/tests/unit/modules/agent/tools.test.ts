/**
 * Unit tests for the tool schemas offered to the model: well-formed, uniquely
 * named, and strict about their arguments.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BROWSER_TOOLS } from '../../../../src/modules/agent/tools.ts';

describe('BROWSER_TOOLS', () => {
  it('names every tool once', () => {
    const names = BROWSER_TOOLS.map((t) => t.function.name);
    assert.equal(new Set(names).size, names.length);
  });

  it('describes each as a function with a closed object schema', () => {
    for (const t of BROWSER_TOOLS) {
      assert.equal(t.type, 'function');
      assert.ok(t.function.description, t.function.name);
      assert.equal(t.function.parameters.type, 'object');
      assert.equal(t.function.parameters.additionalProperties, false, t.function.name);
      for (const r of (t.function.parameters as any).required || []) assert.ok(t.function.parameters.properties[r], r);
    }
  });

  it('only lets the model press safe keys', () => {
    const keys = BROWSER_TOOLS.find((t) => t.function.name === 'press_key').function.parameters.properties.key.enum;
    assert.ok(keys.includes('Enter'));
    assert.ok(!keys.some((k) => /^(F\d+|Meta|Control|Alt|Shift)$/.test(k)));
  });
});
