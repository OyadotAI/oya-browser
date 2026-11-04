/**
 * The OpenAI chat-completions API over a bearer token: OpenAI itself, and every
 * provider with a compatible endpoint (Gemini's /openai path, local servers). The
 * default strategy, serving any base URL no other provider claims.
 */
import type { ChatRequest, LlmProvider } from './types.ts';
import { baseOf, postJson } from './transport.ts';
import { geminiSchema } from './gemini-schema.ts';

/**
 * Google's OpenAI-compatible endpoints (Vertex's /endpoints/openapi, AI Studio's /openai)
 * turn each tool into a Gemini declaration and reject what Gemini does: a tuple `items`
 * is a bare 400 for the whole request. Their tools get the native path's schema. Matched
 * on the host alone, regional Vertex hosts (`us-central1-aiplatform`) included.
 */
const GOOGLE_HOST = /(^|[.-])(aiplatform|generativelanguage)\.googleapis\.com$/;

/** Whether `baseUrl` is served by Google. */
function isGoogle(baseUrl: string) {
  try {
    return GOOGLE_HOST.test(new URL(baseUrl).hostname);
  } catch {
    return false;
  }
}

/** The tools as `baseUrl` takes them: Gemini's schema for Google, as written for everyone else. */
const toolsFor = (baseUrl: string, tools: any[]) =>
  isGoogle(baseUrl)
    ? tools.map((t) => ({ ...t, function: { ...t.function, parameters: geminiSchema(t.function.parameters) } }))
    : tools;

/**
 * Whether the endpoint takes `reasoning_effort` on every model: Gemini's /openai path
 * does; OpenAI's own non-reasoning models (gpt-4o, gpt-4.1) refuse it with a 400.
 */
const takesEffort = (baseUrl: string) => /^https:\/\/generativelanguage\.googleapis\.com\//.test(baseUrl);

/** The chat-completions body; tool_choice defaults to 'auto' when tools are given. */
function bodyFor(r: ChatRequest) {
  const reasoning = r.effort && takesEffort(r.baseUrl) ? { reasoning_effort: r.effort } : {};
  const described = r.tools ? { tools: toolsFor(r.baseUrl, r.tools), tool_choice: r.toolChoice || 'auto' } : {};
  const limit = r.maxTokens ? { max_tokens: r.maxTokens } : {};
  return Object.assign({ model: r.model, messages: r.messages, stream: false }, described, reasoning, limit);
}

/** Any OpenAI-compatible endpoint. */
export const openai: LlmProvider = {
  name: 'openai',
  handles: () => true,
  send: (request) =>
    postJson(
      `${baseOf(request.baseUrl)}/chat/completions`,
      { Authorization: `Bearer ${request.apiKey}`, 'Content-Type': 'application/json' },
      bodyFor(request),
      request.signal,
    ),
};
