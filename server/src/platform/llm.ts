/**
 * One LLM request, OpenAI chat-completions shape in and out.
 *
 * Every provider this control plane supports speaks that shape over a bearer token,
 * except Gemini Enterprise (ex-Vertex AI) in Express mode, whose API keys only
 * authenticate against the native `:generateContent` endpoint; the OpenAI-compatible
 * `.../endpoints/openapi` path wants a Google OAuth access token and answers
 * API_KEY_SERVICE_BLOCKED to an API key. That one provider is translated here so
 * chat-service and playbook keep a single request shape and a single error contract.
 */

import { LLM_ERROR_LOG_CHARS } from './constants.ts';

/**
 * The native Gemini endpoint, recognised by its base URL rather than by llm_provider:
 * a key that pairs llm_provider 'vertex' with a project-scoped /endpoints/openapi base
 * URL and an OAuth token is already speaking OpenAI, and should take the path below.
 */
const isVertexNative = (baseUrl) => /\/publishers\/google\/?$/.test(baseUrl);

/** One chat-completions request, as chatCompletion takes it. */
type ChatRequest = {
  /** Provider base URL; a native Gemini URL switches to the translated path. */
  baseUrl: string;
  /** Provider API key. */
  apiKey: string;
  /** Model name. */
  model: string;
  /** OpenAI-shaped messages. */
  messages: any[];
  /** OpenAI-shaped tool definitions. */
  tools?: any[];
  /** OpenAI tool_choice; defaults to 'auto' when tools are given. */
  toolChoice?: any;
};

/** Sends one chat-completions request and returns an OpenAI-shaped response. Throws on a non-2xx status without echoing the body. */
export async function chatCompletion(request: ChatRequest) {
  const native = isVertexNative(request.baseUrl);
  const res = await post(request, native);
  if (!res.ok) await refuse(res);
  const body = await res.json();
  return native ? fromGemini(body) : body;
}

/** POST the request to the provider, in the shape its endpoint speaks. */
function post(request: ChatRequest, native: boolean) {
  return fetch(endpointFor(request, native), {
    redirect: 'error', // a 30x into an internal address would bypass validateBaseUrl
    method: 'POST',
    headers: headersFor(request.apiKey, native),
    body: JSON.stringify(bodyFor(request, native)),
  });
}

/** JSON, with the key as a bearer token except on the native Gemini path, which carries it in the URL. */
function headersFor(apiKey: string, native: boolean) {
  return { ...(native ? {} : { Authorization: `Bearer ${apiKey}` }), 'Content-Type': 'application/json' };
}

/**
 * Express mode accepts the key only as a query parameter: x-goog-api-key and a bearer
 * token both come back 401 "Expected OAuth 2 access token". That puts a credential in
 * the request URL, so nothing on the failure path may log it.
 */
function endpointFor({ baseUrl, apiKey, model }: ChatRequest, native: boolean) {
  // A base URL is as likely to be written with a trailing slash as without, and
  // the doubled slash that makes is a 404 on at least Google's OpenAI endpoint.
  const base = baseUrl.replace(/\/+$/, '');
  return native
    ? `${base}/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`
    : `${base}/chat/completions`;
}

/** The request body: Gemini's generateContent, or OpenAI chat-completions. */
function bodyFor({ model, messages, tools, toolChoice }: ChatRequest, native: boolean) {
  return native
    ? toGemini({ messages, tools })
    : { model, messages, ...(tools ? { tools, tool_choice: toolChoice || 'auto' } : {}), stream: false };
}

/**
 * The body is not echoed back: the base URL is tenant-configurable, and returning
 * what the endpoint said would turn a misconfigured (or deliberately pointed) URL
 * into a read primitive for the caller.
 */
async function refuse(res: Response): Promise<never> {
  console.error(`[llm] endpoint ${res.status}: ${(await res.text()).slice(0, LLM_ERROR_LOG_CHARS)}`);
  throw new Error(`LLM endpoint returned ${res.status}`);
}

// ── Gemini native translation ──

/**
 * Gemini keys a tool result by function name, where OpenAI keys it by call id. Rather
 * than hold a side map, which the caller's context trimming would invalidate mid-run,
 * the name is carried in the id itself. Tool names are identifiers, so ':' is free.
 */
const callId = (name, index) => `${name}:${index}`;
/** The tool name carried in a call id made by callId. */
const callName = (id) => String(id).split(':')[0];

/** Gemini's function declarations take an OpenAPI subset and reject the rest outright. */
const UNSUPPORTED = new Set(['additionalProperties', 'default', '$schema']);
/** A JSON schema with the keys Gemini rejects removed, recursively. */
function schemaFor(value) {
  if (Array.isArray(value)) return value.map(schemaFor);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !UNSUPPORTED.has(key))
      .map(([key, inner]) => [key, schemaFor(inner)]),
  );
}

/** An OpenAI chat request as a Gemini generateContent body: system prompts become systemInstruction, tool results are grouped per turn. */
export function toGemini({ messages = [], tools }) {
  const contents = [];
  for (const message of messages) addTurn(contents, message);
  return { contents, ...systemInstruction(messages), ...functionDeclarations(tools) };
}

/** Append one non-system message to the Gemini turns. */
function addTurn(contents, message) {
  if (message.role === 'system') return;
  if (message.role === 'tool') return addToolResult(contents, message);
  const parts = messageParts(message);
  if (parts.length) contents.push({ role: message.role === 'assistant' ? 'model' : 'user', parts });
}

/** A tool result as a functionResponse part. One assistant turn can call several tools; Gemini wants their results together. */
function addToolResult(contents, message) {
  const part = {
    functionResponse: { name: callName(message.tool_call_id), response: { result: String(message.content ?? '') } },
  };
  const previous = contents.at(-1);
  if (previous?.role === 'user' && previous.parts.every((p) => p.functionResponse)) previous.parts.push(part);
  else contents.push({ role: 'user', parts: [part] });
}

/** A message's text and tool calls as Gemini parts. */
function messageParts(message) {
  const parts: any[] = contentParts(message.content);
  for (const call of message.tool_calls || []) parts.push(functionCallPart(call));
  return parts;
}

/** OpenAI message content, a string or text and image parts, as Gemini parts. */
function contentParts(content) {
  if (!Array.isArray(content)) return content ? [{ text: content }] : [];
  return content.map((part) =>
    part.type === 'image_url' ? imagePart(part.image_url?.url) : { text: part.text || '' },
  );
}

/** A base64 data-URL image as Gemini inline data. */
function imagePart(url) {
  const match = /^data:([^;,]+);base64,(.*)$/s.exec(String(url || ''));
  return match ? { inlineData: { mimeType: match[1], data: match[2] } } : { text: '' };
}

/** One OpenAI tool call as a Gemini functionCall part. */
function functionCallPart(call) {
  return {
    functionCall: { name: call.function?.name, args: parseArguments(call.function?.arguments) },
    // Gemini 3 rejects the next turn unless each call's thought signature comes
    // back exactly as it was issued.
    ...(call.extra_content?.thoughtSignature ? { thoughtSignature: call.extra_content.thoughtSignature } : {}),
  };
}

/** A tool call's JSON arguments; empty when they do not parse. */
function parseArguments(raw) {
  try {
    return JSON.parse(raw || '{}');
  } catch {
    return {};
  }
}

/** The system prompts, joined, as Gemini's systemInstruction; nothing when there are none. */
function systemInstruction(messages) {
  const system = messages
    .filter((m) => m.role === 'system')
    .map((m) => m.content)
    .filter(Boolean);
  return system.length ? { systemInstruction: { parts: [{ text: system.join('\n\n') }] } } : {};
}

/** OpenAI tool definitions as Gemini function declarations; nothing when there are none. */
function functionDeclarations(tools) {
  if (!tools?.length) return {};
  return { tools: [{ functionDeclarations: tools.map(declarationFor) }] };
}

/** One OpenAI tool as a Gemini function declaration, its schema trimmed to what Gemini accepts. */
function declarationFor(t) {
  return { name: t.function.name, description: t.function.description, parameters: schemaFor(t.function.parameters) };
}

/** A Gemini generateContent response in OpenAI chat-completions shape, including tool calls and token usage. */
export function fromGemini(body) {
  const parts = body.candidates?.[0]?.content?.parts || [];
  const toolCalls = parts.filter((p) => p.functionCall).map(toolCallFrom);
  return {
    choices: [{ message: assistantMessage(textOf(parts), toolCalls) }],
    usage: tokenUsage(body.usageMetadata),
  };
}

/** Gemini's token counts in OpenAI's usage shape, zero when absent. */
function tokenUsage(meta) {
  return { prompt_tokens: meta?.promptTokenCount || 0, completion_tokens: meta?.candidatesTokenCount || 0 };
}

/** The text parts of a Gemini response, concatenated. */
function textOf(parts) {
  return parts
    .filter((p) => p.text)
    .map((p) => p.text)
    .join('');
}

/** A Gemini functionCall part as an OpenAI tool call, its id carrying the tool name. */
function toolCallFrom(p, index) {
  return {
    id: callId(p.functionCall.name, index),
    type: 'function',
    function: { name: p.functionCall.name, arguments: JSON.stringify(p.functionCall.args || {}) },
    ...(p.thoughtSignature ? { extra_content: { thoughtSignature: p.thoughtSignature } } : {}),
  };
}

/** The assistant message: text or null, with tool calls when there are any. */
function assistantMessage(text, toolCalls) {
  return { role: 'assistant', content: text || null, ...(toolCalls.length ? { tool_calls: toolCalls } : {}) };
}
