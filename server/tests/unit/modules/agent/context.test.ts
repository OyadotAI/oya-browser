/**
 * Unit tests for trimming a long run's conversation: the oldest tool results go
 * first, with the assistant turn that asked for them, and the task always stays.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { trimContext } from '../../../../src/modules/agent/context.ts';
import { MAX_CONTEXT_CHARS } from '../../../../src/modules/agent/constants.ts';

const BIG = 'x'.repeat(MAX_CONTEXT_CHARS / 2);

/** An assistant turn calling `n` tools, and their results. */
const toolTurn = (tag, n = 1, content = BIG) => [
  { role: 'assistant', content: null, tool_calls: Array.from({ length: n }, (_, i) => ({ id: `${tag}${i}` })) },
  ...Array.from({ length: n }, (_, i) => ({ role: 'tool', tool_call_id: `${tag}${i}`, content })),
];

const HEAD = [
  { role: 'system', content: 'sys' },
  { role: 'user', content: 'task' },
];

describe('trimContext', () => {
  it('leaves a conversation under the limit alone', () => {
    const messages = [...HEAD, ...toolTurn('a', 1, 'small')];
    trimContext(messages);
    assert.equal(messages.length, 4);
  });

  it('drops the oldest tool turn, with every result it got, until it fits', () => {
    const messages = [...HEAD, ...toolTurn('a', 2, BIG), ...toolTurn('b', 1, 'recent')];
    trimContext(messages);
    assert.deepEqual(
      messages.map((m) => m.tool_call_id || m.role),
      ['system', 'user', 'assistant', 'b0'],
    );
  });

  it('drops a lone tool result that has no assistant turn before it', () => {
    const messages = [
      ...HEAD,
      { role: 'tool', content: BIG },
      { role: 'tool', content: BIG },
      { role: 'user', content: 'x' },
    ];
    trimContext(messages);
    assert.equal(messages.filter((m) => m.role === 'tool').length, 1);
  });

  it('stops when there are no tool results left to drop', () => {
    const messages = [...HEAD, { role: 'user', content: BIG + BIG + BIG }];
    trimContext(messages);
    assert.equal(messages.length, 3);
  });

  it('never trims below the system prompt and the task', () => {
    const messages = [
      { role: 'system', content: 'sys' },
      { role: 'tool', content: BIG + BIG + BIG },
    ];
    trimContext(messages);
    assert.equal(messages.length, 2);
  });
});
