/** Terminal layout must preserve identifiers and measure the rows needed for safe cursor movement. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cellWidth, displayText, wrapProse, terminalRows } from '../../../src/prompt/layout.ts';

describe('setup layout', () => {
  it('wraps prose at both supported widths without dropping words', () => {
    const text = 'The cluster that receives Oya uses a selected Kubernetes context and a dedicated namespace.';
    for (const columns of [36, 72]) {
      const lines = wrapProse(text, columns);
      assert.equal(lines.join(' '), text);
      for (const line of lines) assert.ok(cellWidth(line) <= columns);
    }
  });

  it('keeps a long command or identifier intact and counts native wrapped rows', () => {
    const context = 'arn:aws:eks:us-east-1:123456789012:cluster/long-context-name';
    assert.deepEqual(wrapProse(context, 36), [context]);
    assert.equal(terminalRows(context, 40), 2);
  });

  it('counts combining characters, wide text and joined emoji by displayed cells', () => {
    assert.equal(cellWidth('e\u0301'), 1);
    assert.equal(cellWidth('路径'), 4);
    assert.equal(cellWidth('👩‍💻'), 2);
    assert.equal(cellWidth('🇺🇸'), 2);
    assert.equal(cellWidth('1️⃣'), 2);
  });

  it('displays answers without interpreting terminal control sequences', () => {
    assert.equal(displayText('\u001b[31mvalue\u001b[0m\u0007'), 'value');
  });
});
