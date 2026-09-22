/**
 * Unit tests for platform/llm/anthropic-messages.ts: the shared message shape
 * to and from Claude's Messages API.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { messagesOf, systemOf, toolsOf, fromClaude } from '../../../../src/platform/llm/anthropic-messages.ts';

/** An assistant turn that called `ids`, as the loop stores it, with Claude's own blocks when `own` is given. */
const called = (ids: string[], own?: any[]) => ({
  role: 'assistant',
  content: null,
  tool_calls: ids.map((id) => ({ id, type: 'function', function: { name: 'click', arguments: '{"element_id":1}' } })),
  ...(own ? { extra_content: { anthropic: own } } : {}),
});

describe('to Claude', () => {
  it('sends the system prompts as one cached block', () => {
    const system = systemOf([
      { role: 'system', content: 'a' },
      { role: 'user', content: 'x' },
      { role: 'system', content: 'b' },
    ]);
    assert.deepEqual(system, [{ type: 'text', text: 'a\n\nb', cache_control: { type: 'ephemeral' } }]);
  });

  it('declares each tool with its JSON schema as the input schema', () => {
    const tool = {
      type: 'function',
      function: { name: 'click', description: 'Click', parameters: { type: 'object' } },
    };
    assert.deepEqual(toolsOf([tool]), [{ name: 'click', description: 'Click', input_schema: { type: 'object' } }]);
  });

  it('returns a turn’s tool results together in one user message, with a screenshot after them', () => {
    const turns = messagesOf([
      { role: 'user', content: 'go' },
      called(['a', 'b']),
      { role: 'tool', tool_call_id: 'a', content: 'ok' },
      { role: 'tool', tool_call_id: 'b', content: 'ok' },
      { role: 'user', content: [{ type: 'image_url', image_url: { url: 'data:image/jpeg;base64,QUJD' } }] },
    ]);
    assert.deepEqual(
      turns.map((t) => t.role),
      ['user', 'assistant', 'user'],
    );
    assert.deepEqual(
      (turns[2].content as any[]).map((b) => b.type),
      ['tool_result', 'tool_result', 'image'],
    );
    assert.deepEqual((turns[2].content as any[])[2].source, { type: 'base64', media_type: 'image/jpeg', data: 'QUJD' });
  });

  it('repeats Claude’s own blocks, thinking included, for the latest turn only', () => {
    const thinking = { type: 'thinking', thinking: '', signature: 's' };
    const own = (id: string) => [thinking, { type: 'tool_use', id, name: 'click', input: { element_id: 1 } }];
    const turns = messagesOf([
      { role: 'user', content: 'go' },
      called(['a'], own('a')),
      { role: 'tool', tool_call_id: 'a', content: 'ok' },
      called(['b'], own('b')),
      { role: 'tool', tool_call_id: 'b', content: 'ok' },
    ]);
    assert.deepEqual(
      (turns[1].content as any[]).map((b) => b.type),
      ['tool_use'],
    );
    assert.deepEqual(
      (turns[3].content as any[]).map((b) => b.type),
      ['thinking', 'tool_use'],
    );
  });

  it('drops empty text, which Claude rejects', () => {
    assert.deepEqual(
      messagesOf([
        { role: 'user', content: '' },
        { role: 'user', content: 'hi' },
      ]),
      [{ role: 'user', content: [{ type: 'text', text: 'hi' }] }],
    );
  });
});

describe('from Claude', () => {
  it('turns tool_use blocks into tool calls and keeps Claude’s blocks for the next turn', () => {
    const content = [
      { type: 'thinking', thinking: '', signature: 's' },
      { type: 'text', text: 'Clicking.' },
      { type: 'tool_use', id: 't1', name: 'click', input: { element_id: 3 } },
    ];
    const { message } = fromClaude({ content, stop_reason: 'tool_use', usage: {} }).choices[0];
    assert.equal(message.content, 'Clicking.');
    assert.deepEqual(message.tool_calls, [
      { id: 't1', type: 'function', function: { name: 'click', arguments: '{"element_id":3}' } },
    ]);
    assert.equal(message.extra_content.anthropic, content);
  });

  it('bills cached reads and cache writes as input', () => {
    const usage = { input_tokens: 10, cache_read_input_tokens: 100, cache_creation_input_tokens: 5, output_tokens: 7 };
    assert.deepEqual(fromClaude({ content: [], usage }).usage, { prompt_tokens: 115, completion_tokens: 7 });
  });

  it('reports a declined request as a failure, which ends the run instead of looping', () => {
    const out = fromClaude({ content: [], stop_reason: 'refusal', stop_details: { category: 'cyber' }, usage: {} });
    assert.equal(out.choices[0].message.content, 'FAILED: the model declined this request (cyber).');
  });
});
