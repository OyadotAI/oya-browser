/**
 * Gemini's native generateContent API: Gemini Enterprise (ex-Vertex AI) in Express
 * mode, whose API keys only authenticate against `:generateContent` (the
 * OpenAI-compatible `.../endpoints/openapi` path wants a Google OAuth token and
 * answers API_KEY_SERVICE_BLOCKED to an API key). Messages are translated to and from
 * the shared OpenAI shape, including images and Gemini 3's thought signatures.
 */
import type { ChatRequest, LlmProvider } from './types.ts';
import { baseOf, postJson } from './transport.ts';

/**
 * The native Gemini endpoint, recognised by its base URL rather than by llm_provider:
 * a key that pairs llm_provider 'vertex' with a project-scoped /endpoints/openapi base
 * URL and an OAuth token is already speaking OpenAI, and should take that path instead.
 */
const isNative = (baseUrl: string) => /\/publishers\/google\/?$/.test(baseUrl);

/**
 * Express mode accepts the key only as a query parameter: x-goog-api-key and a bearer
 * token both come back 401 "Expected OAuth 2 access token". That puts a credential in
 * the request URL, so nothing on the failure path may log it.
 */
const endpointFor = ({ baseUrl, apiKey, model }: ChatRequest) =>
  `${baseOf(baseUrl)}/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`;

/** Gemini's native API. */
export const gemini: LlmProvider = {
  name: 'gemini',
  handles: isNative,
  send: async (request) =>
    fromGemini(await postJson(endpointFor(request), { 'Content-Type': 'application/json' }, toGemini(request))),
};

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

/** What toGemini reads from a request: the messages, and the tools when there are any. */
type GeminiInput = Pick<ChatRequest, 'tools'> & Partial<Pick<ChatRequest, 'messages'>>;

/** An OpenAI chat request as a Gemini generateContent body: system prompts become systemInstruction, tool results are grouped per turn. */
export function toGemini({ messages = [], tools }: GeminiInput) {
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
