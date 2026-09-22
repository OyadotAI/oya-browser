/**
 * The OpenAI chat-completions API over a bearer token: OpenAI itself, and every
 * provider with a compatible endpoint (Gemini's /openai path, local servers). The
 * default strategy, serving any base URL no other provider claims.
 */
import type { ChatRequest, LlmProvider } from './types.ts';
import { baseOf, postJson } from './transport.ts';

/** The chat-completions body; tool_choice defaults to 'auto' when tools are given. */
function bodyFor({ model, messages, tools, toolChoice }: ChatRequest) {
  return { model, messages, ...(tools ? { tools, tool_choice: toolChoice || 'auto' } : {}), stream: false };
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
