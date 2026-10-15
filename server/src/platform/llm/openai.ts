/**
 * The OpenAI chat-completions API over a bearer token: OpenAI itself, and every
 * provider with a compatible endpoint (Gemini's /openai path, local servers). The
 * default strategy, serving any base URL no other provider claims.
 */
import type { ChatRequest, LlmProvider } from './types.ts';
import { baseOf, postJson } from './transport.ts';

/**
 * Whether the endpoint takes `reasoning_effort` on every model: Gemini's /openai path
 * does; OpenAI's own non-reasoning models (gpt-4o, gpt-4.1) refuse it with a 400.
 */
const takesEffort = (baseUrl: string) => /^https:\/\/generativelanguage\.googleapis\.com\//.test(baseUrl);

/** The chat-completions body; tool_choice defaults to 'auto' when tools are given. */
function bodyFor({ model, messages, tools, toolChoice, effort, baseUrl }: ChatRequest) {
  const reasoning = effort && takesEffort(baseUrl) ? { reasoning_effort: effort } : {};
  return {
    model,
    messages,
    ...(tools ? { tools, tool_choice: toolChoice || 'auto' } : {}),
    ...reasoning,
    stream: false,
  };
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
    ),
};
