/**
 * One model request, whichever provider serves it. The provider is picked here, in
 * one place, by the base URL; every one of them goes through the same retry policy
 * and answers in the same OpenAI shape.
 */
import type { ChatRequest, ChatResponse, LlmProvider } from './types.ts';
import { anthropic } from './anthropic.ts';
import { gemini } from './gemini.ts';
import { openai } from './openai.ts';
import { withRetries } from './transport.ts';

/** The providers, most specific first: the OpenAI-compatible one takes any base URL the others do not claim. */
const PROVIDERS: LlmProvider[] = [anthropic, gemini, openai];

/** The provider that serves `baseUrl`. */
export const providerFor = (baseUrl: string) => PROVIDERS.find((p) => p.handles(baseUrl)) as LlmProvider;

/** Sends one request, retried on rate limits, overloads and dropped connections; throws LlmError when it fails for good. */
export function chatCompletion(request: ChatRequest): Promise<ChatResponse> {
  const provider = providerFor(request.baseUrl);
  return withRetries(() => provider.send(request));
}

export { toGemini, fromGemini } from './gemini.ts';
export { LlmError } from './transport.ts';
export type { ChatRequest, ChatResponse, LlmProvider } from './types.ts';
