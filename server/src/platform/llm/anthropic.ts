/**
 * Claude, through the official SDK and the Messages API: adaptive thinking between
 * tool calls, parallel tool calls, prompt caching of the tools and system prompt, and
 * server-side refusal fallbacks. The SDK's own retries are off: transport.ts retries
 * every provider the same way.
 */
import Anthropic from '@anthropic-ai/sdk';
import type { ChatRequest, LlmProvider } from './types.ts';
import { LlmError, statusError, unanswered } from './transport.ts';
import { systemOf, toolsOf, messagesOf, fromClaude } from './anthropic-messages.ts';
import { CLAUDE_EFFORT, CLAUDE_MAX_TOKENS, LLM_TIMEOUT_MS } from '../constants.ts';

/** Claude's own API host: a base URL there is sent to the Messages API, not its OpenAI-compatible shim. */
const HOST = 'api.anthropic.com';

/** Whether `baseUrl` is Anthropic's API. */
function isAnthropic(baseUrl: string) {
  try {
    return new URL(baseUrl).hostname === HOST;
  } catch {
    return false;
  }
}

/** The SDK wants the origin; the configured base usually ends in /v1, which the SDK adds itself. */
const originOf = (baseUrl: string) => new URL(baseUrl).origin;

/** How Claude works each step: thinking as it needs, at the configured effort, with room for its reply. */
const STEP = {
  max_tokens: CLAUDE_MAX_TOKENS,
  thinking: { type: 'adaptive' as const },
  output_config: { effort: CLAUDE_EFFORT as any },
};

/** Haiku 4.5 takes neither adaptive thinking nor effort (both are a 400): it runs without them. */
const HAIKU_STEP = { max_tokens: CLAUDE_MAX_TOKENS };

/** The per-step settings `model` accepts. */
const stepFor = (model: string) => (/haiku/.test(model) ? HAIKU_STEP : STEP);

/**
 * What every request carries: a rolling cache breakpoint on the last block, so each
 * step reuses the run so far, and on a policy decline a retry on the API's default
 * fallback model inside the same call.
 */
const ALWAYS = {
  cache_control: { type: 'ephemeral' as const },
  betas: ['server-side-fallback-2026-07-01'],
  fallbacks: 'default' as const,
};

/** The Messages API request for one step of the loop. */
function paramsFor({ model, messages, tools }: ChatRequest) {
  const conversation = { system: systemOf(messages), messages: messagesOf(messages) };
  const described = tools?.length ? { tools: toolsOf(tools) } : {};
  return { model, ...stepFor(model), ...conversation, ...described, ...ALWAYS };
}

/** An SDK failure in the shared contract: its status, or none when it never got an answer. */
function failure(err: any): LlmError {
  if (!(err instanceof Anthropic.APIError) || err.status == null) return unanswered(err);
  return statusError(err.status, String(err.message), err.headers?.get?.('retry-after'));
}

/** fetch that refuses a redirect, as transport.ts does for every other provider: a 30x could lead into an internal address. */
const noRedirects = (url, init) => fetch(url, { ...init, redirect: 'error' });

/** A client for one request, its own retries off (transport.ts retries every provider alike). */
const clientFor = (request: ChatRequest) =>
  new Anthropic({
    apiKey: request.apiKey,
    baseURL: originOf(request.baseUrl),
    maxRetries: 0,
    timeout: LLM_TIMEOUT_MS,
    fetch: noRedirects,
  });

/** Claude's Messages API. */
export const anthropic: LlmProvider = {
  name: 'anthropic',
  handles: isAnthropic,
  async send(request) {
    const response = await clientFor(request)
      .beta.messages.create(paramsFor(request) as any)
      .catch((err) => {
        throw failure(err);
      });
    return fromClaude(response);
  },
};
