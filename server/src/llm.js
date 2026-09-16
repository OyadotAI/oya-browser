/**
 * One LLM request, OpenAI chat-completions shape in and out.
 *
 * Every provider this control plane supports speaks that shape over a bearer token —
 * except Gemini Enterprise (ex-Vertex AI) in Express mode, whose API keys only
 * authenticate against the native `:generateContent` endpoint; the OpenAI-compatible
 * `.../endpoints/openapi` path wants a Google OAuth access token and answers
 * API_KEY_SERVICE_BLOCKED to an API key. That one provider is translated here so
 * chat-service and playbook keep a single request shape and a single error contract.
 */

/**
 * The native Gemini endpoint, recognised by its base URL rather than by llm_provider:
 * a key that pairs llm_provider 'vertex' with a project-scoped /endpoints/openapi base
 * URL and an OAuth token is already speaking OpenAI, and should take the path below.
 */
const isVertexNative = (baseUrl) => /\/publishers\/google\/?$/.test(baseUrl);

export async function chatCompletion({ baseUrl, apiKey, model, messages, tools, toolChoice }) {
  const native = isVertexNative(baseUrl);
  // Express mode accepts the key only as a query parameter: x-goog-api-key and a bearer
  // token both come back 401 "Expected OAuth 2 access token". That puts a credential in
  // the request URL, so nothing on the failure path below may log it.
  const url = native
    ? `${baseUrl.replace(/\/+$/, '')}/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`
    : `${baseUrl}/chat/completions`;
  const res = await fetch(url,
    {
      redirect: 'error', // a 30x into an internal address would bypass validateBaseUrl
      method: 'POST',
      headers: {
        ...(native ? {} : { Authorization: `Bearer ${apiKey}` }),
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(native
        ? toGemini({ messages, tools })
        : { model, messages, ...(tools ? { tools, tool_choice: toolChoice || 'auto' } : {}), stream: false }),
    });

  if (!res.ok) {
    // The body is not echoed back: the base URL is tenant-configurable, and returning
    // what the endpoint said would turn a misconfigured (or deliberately pointed) URL
    // into a read primitive for the caller.
    console.error(`[llm] endpoint ${res.status}: ${(await res.text()).slice(0, 500)}`);
    throw new Error(`LLM endpoint returned ${res.status}`);
  }
  const body = await res.json();
  return native ? fromGemini(body) : body;
}

// ── Gemini native translation ──

/**
 * Gemini keys a tool result by function name, where OpenAI keys it by call id. Rather
 * than hold a side map — which the caller's context trimming would invalidate mid-run —
 * the name is carried in the id itself. Tool names are identifiers, so ':' is free.
 */
const callId = (name, index) => `${name}:${index}`;
const callName = (id) => String(id).split(':')[0];

/** Gemini's function declarations take an OpenAPI subset and reject the rest outright. */
const UNSUPPORTED = new Set(['additionalProperties', 'default', '$schema']);
function schemaFor(value) {
  if (Array.isArray(value)) return value.map(schemaFor);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(Object.entries(value)
    .filter(([key]) => !UNSUPPORTED.has(key))
    .map(([key, inner]) => [key, schemaFor(inner)]));
}

export function toGemini({ messages = [], tools }) {
  const system = messages.filter((m) => m.role === 'system').map((m) => m.content).filter(Boolean);
  const contents = [];
  for (const message of messages) {
    if (message.role === 'system') continue;

    if (message.role === 'tool') {
      const part = { functionResponse: { name: callName(message.tool_call_id), response: { result: String(message.content ?? '') } } };
      // One assistant turn can call several tools; Gemini wants their results together.
      const previous = contents.at(-1);
      if (previous?.role === 'user' && previous.parts.every((p) => p.functionResponse)) previous.parts.push(part);
      else contents.push({ role: 'user', parts: [part] });
      continue;
    }

    const parts = [];
    if (message.content) parts.push({ text: message.content });
    for (const call of message.tool_calls || []) {
      let args = {};
      try { args = JSON.parse(call.function?.arguments || '{}'); } catch {}
      parts.push({
        functionCall: { name: call.function?.name, args },
        // Gemini 3 rejects the next turn unless each call's thought signature comes
        // back exactly as it was issued.
        ...(call.extra_content?.thoughtSignature ? { thoughtSignature: call.extra_content.thoughtSignature } : {}),
      });
    }
    if (parts.length) contents.push({ role: message.role === 'assistant' ? 'model' : 'user', parts });
  }

  return {
    contents,
    ...(system.length ? { systemInstruction: { parts: [{ text: system.join('\n\n') }] } } : {}),
    ...(tools?.length ? { tools: [{ functionDeclarations: tools.map((t) => ({
      name: t.function.name,
      description: t.function.description,
      parameters: schemaFor(t.function.parameters),
    })) }] } : {}),
  };
}

export function fromGemini(body) {
  const parts = body.candidates?.[0]?.content?.parts || [];
  const text = parts.filter((p) => p.text).map((p) => p.text).join('');
  const toolCalls = parts.filter((p) => p.functionCall).map((p, index) => ({
    id: callId(p.functionCall.name, index),
    type: 'function',
    function: { name: p.functionCall.name, arguments: JSON.stringify(p.functionCall.args || {}) },
    ...(p.thoughtSignature ? { extra_content: { thoughtSignature: p.thoughtSignature } } : {}),
  }));
  return {
    choices: [{ message: { role: 'assistant', content: text || null, ...(toolCalls.length ? { tool_calls: toolCalls } : {}) } }],
    usage: {
      prompt_tokens: body.usageMetadata?.promptTokenCount || 0,
      completion_tokens: body.usageMetadata?.candidatesTokenCount || 0,
    },
  };
}
