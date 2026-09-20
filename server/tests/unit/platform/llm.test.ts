/**
 * Unit tests for the LLM request: the OpenAI chat-completions path, the native
 * Gemini path with its request and response translation, and a failed call
 * that never echoes the provider's body.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { chatCompletion, fromGemini, toGemini } from '../../../src/platform/llm.ts';

/** Stubs fetch to answer `body` with `status`; returns the mock to read the calls from. */
function answer(body: unknown, status = 200) {
  return mock.method(globalThis, 'fetch', async () => new Response(JSON.stringify(body), { status }));
}

/** The URL and parsed init of the nth fetch call. */
function call(fetch, n = 0) {
  const [url, init] = fetch.mock.calls[n].arguments;
  return { url, init, body: JSON.parse(init.body) };
}

const GEMINI_BASE = 'https://aiplatform.googleapis.com/v1/publishers/google';

describe('chatCompletion', () => {
  afterEach(() => mock.restoreAll());

  it('posts an OpenAI request with a bearer token and returns the body as is', async () => {
    const fetch = answer({ choices: [{ message: { content: 'hi' } }] });
    const out = await chatCompletion({ baseUrl: 'https://api.test/v1', apiKey: 'sk-1', model: 'm', messages: [] });
    const { url, init, body } = call(fetch);
    assert.equal(url, 'https://api.test/v1/chat/completions');
    assert.equal(init.headers.Authorization, 'Bearer sk-1');
    assert.equal(init.redirect, 'error');
    assert.deepEqual(body, { model: 'm', messages: [], stream: false });
    assert.equal(out.choices[0].message.content, 'hi');
  });

  it('sends tools with tool_choice auto unless one is given', async () => {
    const fetch = answer({});
    const tools = [{ type: 'function', function: { name: 't' } }];
    await chatCompletion({ baseUrl: 'https://api.test', apiKey: 'k', model: 'm', messages: [], tools });
    await chatCompletion({
      baseUrl: 'https://api.test',
      apiKey: 'k',
      model: 'm',
      messages: [],
      tools,
      toolChoice: 'none',
    });
    assert.equal(call(fetch, 0).body.tool_choice, 'auto');
    assert.equal(call(fetch, 1).body.tool_choice, 'none');
  });

  it('throws only the status on a failed call, never the provider’s body', async () => {
    answer({ error: 'internal secret detail' }, 500);
    const logged = mock.method(console, 'error', () => {});
    await assert.rejects(
      chatCompletion({ baseUrl: 'https://api.test', apiKey: 'k', model: 'm', messages: [] }),
      (err: Error) => err.message === 'LLM endpoint returned 500',
    );
    assert.equal(logged.mock.callCount(), 1);
  });

  it('calls native Gemini with the key in the URL instead of a bearer header', async () => {
    const fetch = answer({ candidates: [{ content: { parts: [{ text: 'ok' }] } }] });
    const out = await chatCompletion({ baseUrl: `${GEMINI_BASE}/`, apiKey: 'k&1', model: 'gemini 2', messages: [] });
    const { url, init } = call(fetch);
    assert.equal(url, `${GEMINI_BASE}/models/gemini%202:generateContent?key=k%261`);
    assert.equal(init.headers.Authorization, undefined);
    assert.equal(out.choices[0].message.content, 'ok');
  });
});

describe('toGemini images', () => {
  it('sends a screenshot turn as inline image data beside its text', () => {
    const content = [
      { type: 'text', text: 'Screenshot:' },
      { type: 'image_url', image_url: { url: 'data:image/jpeg;base64,QUJD' } },
    ];
    const { contents } = toGemini({ messages: [{ role: 'user', content }] });
    assert.deepEqual(contents[0].parts, [
      { text: 'Screenshot:' },
      { inlineData: { mimeType: 'image/jpeg', data: 'QUJD' } },
    ]);
  });
});

describe('toGemini', () => {
  it('moves system prompts into systemInstruction and maps roles', () => {
    const out = toGemini({
      messages: [
        { role: 'system', content: 'be brief' },
        { role: 'system', content: 'be kind' },
        { role: 'user', content: 'hello' },
        { role: 'assistant', content: 'hi' },
      ],
    });
    assert.deepEqual(out.systemInstruction, { parts: [{ text: 'be brief\n\nbe kind' }] });
    assert.deepEqual(
      out.contents.map((c) => c.role),
      ['user', 'model'],
    );
  });

  it('turns tool calls into functionCall parts, keeping the thought signature', () => {
    const out = toGemini({
      messages: [
        {
          role: 'assistant',
          tool_calls: [
            { function: { name: 'click', arguments: '{"id":3}' }, extra_content: { thoughtSignature: 'sig' } },
            { function: { name: 'look', arguments: 'not json' } },
          ],
        },
      ],
    });
    assert.deepEqual(out.contents[0].parts, [
      { functionCall: { name: 'click', args: { id: 3 } }, thoughtSignature: 'sig' },
      { functionCall: { name: 'look', args: {} } },
    ]);
  });

  it('groups the results of one turn’s tool calls, named from the call id', () => {
    const out = toGemini({
      messages: [
        { role: 'tool', tool_call_id: 'click:0', content: 'done' },
        { role: 'tool', tool_call_id: 'look:1', content: null },
      ],
    });
    assert.equal(out.contents.length, 1);
    assert.deepEqual(out.contents[0].parts, [
      { functionResponse: { name: 'click', response: { result: 'done' } } },
      { functionResponse: { name: 'look', response: { result: '' } } },
    ]);
  });

  it('declares tools with the schema keys Gemini rejects removed at every depth', () => {
    const parameters = {
      type: 'object',
      additionalProperties: false,
      $schema: 'x',
      properties: {
        n: { type: 'number', default: 1 },
        list: { type: 'array', items: [{ default: 2, type: 'string' }] },
      },
    };
    const out = toGemini({ messages: [], tools: [{ function: { name: 't', description: 'd', parameters } }] });
    assert.deepEqual(out.tools[0].functionDeclarations[0], {
      name: 't',
      description: 'd',
      parameters: {
        type: 'object',
        properties: { n: { type: 'number' }, list: { type: 'array', items: [{ type: 'string' }] } },
      },
    });
  });

  it('adds no systemInstruction, tools or empty turns when there are none', () => {
    const out = toGemini({ messages: [{ role: 'user', content: '' }] });
    assert.deepEqual(out, { contents: [] });
  });
});

describe('fromGemini', () => {
  it('joins text parts and reports token usage', () => {
    const out = fromGemini({
      candidates: [{ content: { parts: [{ text: 'a' }, { text: 'b' }] } }],
      usageMetadata: { promptTokenCount: 4, candidatesTokenCount: 2 },
    });
    assert.deepEqual(out, {
      choices: [{ message: { role: 'assistant', content: 'ab' } }],
      usage: { prompt_tokens: 4, completion_tokens: 2 },
    });
  });

  it('turns function calls into tool calls whose id carries the tool name', () => {
    const out = fromGemini({
      candidates: [
        { content: { parts: [{ functionCall: { name: 'click', args: { id: 1 } }, thoughtSignature: 's' }] } },
      ],
    });
    const message: any = out.choices[0].message;
    assert.equal(message.content, null);
    assert.deepEqual(message.tool_calls, [
      {
        id: 'click:0',
        type: 'function',
        function: { name: 'click', arguments: '{"id":1}' },
        extra_content: { thoughtSignature: 's' },
      },
    ]);
  });

  it('answers an empty message with zero usage for an empty response', () => {
    assert.deepEqual(fromGemini({}), {
      choices: [{ message: { role: 'assistant', content: null } }],
      usage: { prompt_tokens: 0, completion_tokens: 0 },
    });
  });
});
