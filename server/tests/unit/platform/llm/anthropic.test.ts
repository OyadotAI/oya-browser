/**
 * Unit tests for platform/llm/anthropic.ts: what one step asks Claude for, and
 * that its failures take the shared error contract (and so the shared retries).
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { anthropic } from '../../../../src/platform/llm/anthropic.ts';
import { LlmError } from '../../../../src/platform/llm/transport.ts';

/** Stubs fetch to answer `body` with `status`; returns the mock to read the request from. */
function answer(body: unknown, status = 200) {
  return mock.method(
    globalThis,
    'fetch',
    async () => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } }),
  );
}

/** A request for one step, with one tool. */
const STEP = {
  baseUrl: 'https://api.anthropic.com/v1',
  apiKey: 'sk-ant',
  model: 'claude-opus-5',
  messages: [
    { role: 'system', content: 'Be careful.' },
    { role: 'user', content: 'Open the page' },
  ],
  tools: [{ type: 'function', function: { name: 'navigate', description: 'Go', parameters: { type: 'object' } } }],
};

/** A reply that calls one tool. */
const REPLY = {
  id: 'msg_1',
  type: 'message',
  role: 'assistant',
  model: 'claude-opus-5',
  content: [{ type: 'tool_use', id: 't1', name: 'navigate', input: { url: 'https://x.test' } }],
  stop_reason: 'tool_use',
  usage: { input_tokens: 5, output_tokens: 3 },
};

describe('anthropic', () => {
  afterEach(() => mock.restoreAll());

  it('asks the Messages API with adaptive thinking, cached prompts and refusal fallbacks', async () => {
    const fetch = answer(REPLY);
    const out = await anthropic.send(STEP);
    const [url, init] = fetch.mock.calls[0].arguments as any[];
    const body = JSON.parse(init.body);
    assert.match(String(url), /^https:\/\/api\.anthropic\.com\/v1\/messages/);
    assert.deepEqual([body.model, body.thinking.type, body.fallbacks], ['claude-opus-5', 'adaptive', 'default']);
    assert.deepEqual(body.cache_control, { type: 'ephemeral' });
    assert.equal(body.system[0].cache_control.type, 'ephemeral');
    assert.equal(body.tools[0].name, 'navigate');
    assert.equal(out.choices[0].message.tool_calls[0].function.name, 'navigate');
  });

  it('sends Haiku without the thinking and effort settings it rejects', async () => {
    const fetch = answer(REPLY);
    await anthropic.send({ ...STEP, model: 'claude-haiku-4-5' });
    const body = JSON.parse((fetch.mock.calls[0].arguments as any[])[1].body);
    assert.deepEqual([body.thinking, body.output_config], [undefined, undefined]);
  });

  it('fails with the status only, as every provider does, so the shared retries apply', async () => {
    mock.method(console, 'error', () => {});
    answer({ type: 'error', error: { type: 'overloaded_error', message: 'secret detail' } }, 529);
    await assert.rejects(
      anthropic.send(STEP),
      (err: LlmError) => err.status === 529 && err.message === 'LLM endpoint returned 529',
    );
  });
});
