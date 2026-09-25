/**
 * Unit tests for platform/llm/index.ts: the provider is picked by base URL, and every
 * provider's failures are retried the same way.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { chatCompletion, providerFor } from '../../../../src/platform/llm/index.ts';

describe('providerFor', () => {
  it('sends Anthropic’s host to Claude, the native Gemini path to Gemini, and anything else to the OpenAI shape', () => {
    assert.equal(providerFor('https://api.anthropic.com/v1').name, 'anthropic');
    assert.equal(providerFor('https://aiplatform.googleapis.com/v1/publishers/google').name, 'gemini');
    assert.equal(providerFor('https://generativelanguage.googleapis.com/v1beta/openai').name, 'openai');
    assert.equal(providerFor('https://api.openai.com/v1').name, 'openai');
    assert.equal(providerFor('https://openrouter.ai/api/v1').name, 'openai');
  });
});

describe('chatCompletion', () => {
  afterEach(() => mock.restoreAll());

  it('retries an overloaded provider and returns its next answer', async () => {
    mock.method(console, 'error', () => {});
    const answers = [
      new Response('busy', { status: 503, headers: { 'retry-after': '0' } }),
      new Response(JSON.stringify({ choices: [{ message: { content: 'hi' } }] }), { status: 200 }),
    ];
    const fetch = mock.method(globalThis, 'fetch', async () => answers.shift());
    const out = await chatCompletion({ baseUrl: 'https://api.test/v1', apiKey: 'k', model: 'm', messages: [] });
    assert.equal(out.choices[0].message.content, 'hi');
    assert.equal(fetch.mock.callCount(), 2);
  });
});
