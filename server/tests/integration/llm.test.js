/**
 * The Gemini-native translation in llm.js. It is pure, so most of this is assertions on
 * shape, but the parts that actually break a run (a tool result finding its way back to
 * the right function, a thought signature surviving a round trip) are only proved by a
 * real multi-turn exchange, so this also runs one against a live endpoint when a key is
 * present. AI Studio's native generateContent is the same wire format as Gemini
 * Enterprise express mode, which is what makes that check possible without a GCP project.
 */
import assert from 'node:assert';
import { toGemini, fromGemini, chatCompletion } from '../../src/platform/llm.ts';
import { BROWSER_TOOLS } from '../../src/modules/agent/tools.ts';
import * as keyConfig from '../../src/modules/config/service.ts';

await keyConfig.set('k-preset', { llm_provider: 'vertex', openai_api_key: 'x' });
const LLM_DEFAULTS_VERTEX = { base: keyConfig.resolve('k-preset').baseUrl, model: keyConfig.resolve('k-preset').model };

// ── Request translation ──

const navigate = BROWSER_TOOLS.find((t) => t.function.name === 'navigate');
const request = toGemini({
  messages: [
    { role: 'system', content: 'be brief' },
    { role: 'user', content: 'go to example.com' },
  ],
  tools: [navigate],
});

assert.deepEqual(
  request.systemInstruction,
  { parts: [{ text: 'be brief' }] },
  'a system message becomes systemInstruction',
);
assert.deepEqual(
  request.contents,
  [{ role: 'user', parts: [{ text: 'go to example.com' }] }],
  'a user message becomes a user content',
);
const declaration = request.tools[0].functionDeclarations[0];
assert.equal(declaration.name, 'navigate', 'a tool becomes a function declaration');
// Gemini rejects the whole request over a key it does not know, and every tool in
// chat-tools.js carries additionalProperties.
assert.ok(
  !('additionalProperties' in declaration.parameters),
  'additionalProperties is stripped from a real tool schema',
);
assert.deepEqual(declaration.parameters.required, ['url'], 'the supported parts of the schema survive');
assert.equal(declaration.parameters.properties.url.type, 'string', 'nested schema survives');

// A system message is not required, and must not produce an empty instruction.
assert.ok(
  !('systemInstruction' in toGemini({ messages: [{ role: 'user', content: 'hi' }] })),
  'no system message means no systemInstruction',
);

// ── Response translation ──

const completion = fromGemini({
  candidates: [
    {
      content: {
        parts: [
          { text: 'going there' },
          { functionCall: { name: 'navigate', args: { url: 'https://example.com' } }, thoughtSignature: 'sig-1' },
        ],
      },
    },
  ],
  usageMetadata: { promptTokenCount: 11, candidatesTokenCount: 5 },
});
const message = completion.choices[0].message;
assert.equal(message.content, 'going there', 'text parts become content');
assert.equal(message.tool_calls.length, 1, 'a functionCall becomes a tool call');
assert.equal(message.tool_calls[0].function.name, 'navigate', 'the tool name survives');
assert.deepEqual(
  JSON.parse(message.tool_calls[0].function.arguments),
  { url: 'https://example.com' },
  'args are serialised as OpenAI arguments',
);
// Metering in chat-service and playbook reads these names.
assert.deepEqual(completion.usage, { prompt_tokens: 11, completion_tokens: 5 }, 'usageMetadata maps to usage');
// A text-only answer must not carry an empty tool_calls array; chat-service treats a
// truthy tool_calls as "keep looping" and would never return.
assert.ok(
  !('tool_calls' in fromGemini({ candidates: [{ content: { parts: [{ text: 'done' }] } }] }).choices[0].message),
  'a text-only reply has no tool_calls',
);

// ── The round trip that actually breaks runs ──

const next = toGemini({
  messages: [
    { role: 'user', content: 'go to example.com' },
    { role: 'assistant', content: message.content, tool_calls: message.tool_calls },
    { role: 'tool', tool_call_id: message.tool_calls[0].id, content: 'Navigated.' },
  ],
});
const call = next.contents[1].parts.find((p) => p.functionCall);
assert.equal(call.thoughtSignature, 'sig-1', 'a thought signature returns unchanged, or Gemini 3 rejects the turn');
// Gemini keys a result by function name where OpenAI keys it by call id, so this is
// where a lost mapping shows up.
assert.deepEqual(
  next.contents[2],
  { role: 'user', parts: [{ functionResponse: { name: 'navigate', response: { result: 'Navigated.' } } }] },
  'a tool result maps back to the function it answers',
);

// Several results from one assistant turn belong in a single content.
const parallel = toGemini({
  messages: [
    {
      role: 'assistant',
      tool_calls: [
        { id: 'navigate:0', function: { name: 'navigate', arguments: '{}' } },
        { id: 'analyze_page:1', function: { name: 'analyze_page', arguments: '{}' } },
      ],
    },
    { role: 'tool', tool_call_id: 'navigate:0', content: 'ok' },
    { role: 'tool', tool_call_id: 'analyze_page:1', content: 'page' },
  ],
});
assert.equal(parallel.contents.length, 2, 'parallel tool results merge into one turn');
assert.deepEqual(
  parallel.contents[1].parts.map((p) => p.functionResponse.name),
  ['navigate', 'analyze_page'],
  'each result keeps its own function',
);

// Malformed arguments must not take the process down mid-run.
assert.deepEqual(
  toGemini({
    messages: [
      { role: 'assistant', tool_calls: [{ id: 'navigate:0', function: { name: 'navigate', arguments: 'not json' } }] },
    ],
  }).contents[0].parts[0].functionCall.args,
  {},
  'unparseable arguments degrade to empty args',
);

// ── Routing: base URL shape picks the path, the header and the body ──

// Express mode is recognised by its base URL rather than by llm_provider, so a key that
// pairs 'vertex' with a project-scoped /endpoints/openapi endpoint and an OAuth token
// still takes the OpenAI path. A stub proves the seam the live check above cannot: the
// URL built, the credential header chosen, and which body shape goes out.
const { createServer } = await import('node:http');
const seen = [];
const stub = createServer((req, res) => {
  let body = '';
  req.on('data', (c) => {
    body += c;
  });
  req.on('end', () => {
    seen.push({ url: req.url, headers: req.headers, body: JSON.parse(body || '{}') });
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(
      JSON.stringify(
        req.url.includes('generateContent')
          ? {
              candidates: [{ content: { parts: [{ text: 'ok' }] } }],
              usageMetadata: { promptTokenCount: 1, candidatesTokenCount: 1 },
            }
          : {
              choices: [{ message: { role: 'assistant', content: 'ok' } }],
              usage: { prompt_tokens: 1, completion_tokens: 1 },
            },
      ),
    );
  });
});
await new Promise((r) => stub.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${stub.address().port}`;
const messages = [{ role: 'user', content: 'hi' }];

const native = await chatCompletion({
  baseUrl: `${origin}/v1/publishers/google`,
  apiKey: 'express-key',
  model: 'gemini-2.5-flash',
  messages,
});
assert.equal(native.choices[0].message.content, 'ok', 'the express-mode reply is translated back');
assert.equal(
  seen[0].url,
  '/v1/publishers/google/models/gemini-2.5-flash:generateContent?key=express-key',
  'express mode builds the native model path and carries the key as a query parameter',
);
// Not a stylistic choice: the header and bearer forms are both refused with a 401.
assert.ok(
  !seen[0].headers.authorization && !seen[0].headers['x-goog-api-key'],
  'express mode sends no credential header',
);
assert.ok(seen[0].body.contents, 'express mode sends the native body');

await chatCompletion({
  baseUrl: `${origin}/v1/projects/p/locations/l/endpoints/openapi`,
  apiKey: 'oauth-token',
  model: 'google/gemini-2.5-flash',
  messages,
});
assert.equal(
  seen[1].url,
  '/v1/projects/p/locations/l/endpoints/openapi/chat/completions',
  'a project-scoped endpoint keeps the OpenAI path',
);
assert.equal(seen[1].headers.authorization, 'Bearer oauth-token', 'a project-scoped endpoint uses the bearer token');
assert.ok(seen[1].body.messages, 'a project-scoped endpoint sends the OpenAI body');

// A trailing slash on the base URL must not produce a doubled path separator.
await chatCompletion({ baseUrl: `${origin}/v1/publishers/google/`, apiKey: 'k', model: 'm', messages });
assert.equal(
  seen[2].url,
  '/v1/publishers/google/models/m:generateContent?key=k',
  'a trailing slash on the base URL is tolerated',
);

// The configured preset must actually route to the native path. These are two constants
// in two files, and an edit to either silently sends express-mode keys down the OpenAI
// path, where they fail with a 401 that looks like a bad key.
await keyConfig.set('k-vertex-routing', { llm_provider: 'vertex', openai_api_key: 'AIza-x' });
const configured = keyConfig.resolve('k-vertex-routing');
seen.length = 0;
await chatCompletion({
  ...configured,
  baseUrl: configured.baseUrl.replace('https://aiplatform.googleapis.com', origin),
  apiKey: 'k',
  messages,
});
assert.ok(
  seen[0].url.includes(':generateContent?key='),
  `the vertex preset (${configured.baseUrl}) routes to the native endpoint`,
);
assert.ok(seen[0].url.includes(configured.model), 'the preset model reaches the path');

stub.close();

// ── Live: the same wire format express mode uses ──

// A real two-turn exchange is the only thing that proves the parts a fixture cannot:
// that Gemini accepts our translated tools, and that a tool result and its thought
// signature come back in a form it will take for the next turn.
// VERTEX_EXPRESS_KEY drives the production path end to end, through chatCompletion and
// the configured preset. GEMINI_API_KEY is a fallback that covers the same translation
// against AI Studio's native endpoint, which is the identical wire format.
const express = process.env.VERTEX_EXPRESS_KEY;
const studio = process.env.GEMINI_API_KEY;

if (express || studio) {
  const preset = LLM_DEFAULTS_VERTEX;
  const call = express
    ? (messages) =>
        chatCompletion({ baseUrl: preset.base, apiKey: express, model: preset.model, messages, tools: [navigate] })
    : async (messages) => {
        const res = await fetch(
          'https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent',
          {
            method: 'POST',
            headers: { 'x-goog-api-key': studio, 'Content-Type': 'application/json' },
            body: JSON.stringify(toGemini({ messages, tools: [navigate] })),
          },
        );
        if (!res.ok)
          assert.fail(
            `live: Gemini rejected the translated request (${res.status}: ${(await res.text()).slice(0, 300)})`,
          );
        return fromGemini(await res.json());
      };

  const history = [
    { role: 'system', content: 'Use the navigate tool. Do not ask questions.' },
    { role: 'user', content: 'Open https://example.com' },
  ];
  const first = await call(history);
  const calls = first.choices[0].message.tool_calls;
  assert.ok(
    calls?.length,
    `live: the model asked for a tool call (got ${JSON.stringify(first.choices[0].message).slice(0, 200)})`,
  );
  assert.equal(calls[0].function.name, 'navigate', 'live: it picked the tool we declared');
  assert.equal(
    JSON.parse(calls[0].function.arguments).url,
    'https://example.com',
    'live: the arguments survived translation',
  );
  assert.ok(first.usage.prompt_tokens > 0, 'live: usage is reported');

  // The turn that fails if a signature or a tool name is lost, and it is Gemini
  // rejecting it, not an assertion of ours.
  const second = await call([
    ...history,
    { role: 'assistant', content: first.choices[0].message.content, tool_calls: calls },
    ...calls.map((c) => ({
      role: 'tool',
      tool_call_id: c.id,
      content: 'Navigated to https://example.com. The page title is "Example Domain".',
    })),
    { role: 'user', content: 'What is the page title?' },
  ]);
  assert.ok(second.choices[0].message.content, 'live: the model answered from the tool result');
  console.log(
    `  live ${express ? `express mode (${preset.base})` : 'AI Studio native'} round trip ok, ${JSON.stringify(second.choices[0].message.content).slice(0, 80)}`,
  );
} else {
  console.log('  (set VERTEX_EXPRESS_KEY, or GEMINI_API_KEY, for the live round-trip check)');
}

console.log(
  'LLM translation passed: system instruction, schema sanitising, tool calls, thought signatures, tool-result mapping, parallel results, usage metering, express-mode vs project-scoped routing.',
);
