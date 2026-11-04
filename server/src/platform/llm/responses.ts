/**
 * OpenAI's Responses API, which a ChatGPT plan answers on (Sign in with ChatGPT):
 * a base URL ending in /responses is posted to as is. The plan requires
 * `store: false` and `stream: true`, and a request counts only once the stream says
 * `response.completed`; a usage-limit failure can arrive after streaming started.
 */
import type { ChatRequest, LlmProvider } from './types.ts';
import { baseOf, LlmError, refuse, statusError, unanswered } from './transport.ts';
import { fromResponses, inputOf, instructionsOf, toolChoiceOf, toolsOf } from './responses-messages.ts';
import { LLM_TIMEOUT_MS } from '../constants.ts';
import { Status } from '../http-status.ts';

/** Failure codes that mean the plan's allowance is spent: retrying will not help until it resets. */
const USAGE_SPENT = ['subscription_sharing_usage_limit_exceeded', 'subscription_sharing_usage_unavailable'];

/** Whether `baseUrl` is a Responses endpoint. */
const isResponses = (baseUrl: string) => /\/responses$/.test(baseOf(baseUrl || ''));

/** What the plan requires of every request: nothing stored, the answer streamed. */
const PLAN = { store: false, stream: true };

/** The request body: the conversation as instructions and input items, and the tools when there are any. */
function bodyFor({ model, messages, tools, toolChoice, maxTokens }: ChatRequest) {
  const described = tools?.length ? { tools: toolsOf(tools), tool_choice: toolChoiceOf(toolChoice) } : {};
  const limit = maxTokens ? { max_output_tokens: maxTokens } : {};
  return { model, instructions: instructionsOf(messages), input: inputOf(messages), ...described, ...limit, ...PLAN };
}

/** One event's JSON data, or nothing when it is not JSON (a keep-alive, a [DONE] marker). */
function parsed(data: string): any[] {
  try {
    return [JSON.parse(data)];
  } catch {
    return [];
  }
}

/** The server-sent events in a finished stream. */
const eventsOf = (text: string): any[] =>
  text
    .split('\n')
    .filter((line) => line.startsWith('data:'))
    .flatMap((line) => parsed(line.slice('data:'.length).trim()));

/** Why a stream ended without completing, as the shared error: spent allowance is final, the rest is retried. */
function failureOf(event: any): LlmError {
  const code = event?.response?.error?.code || event?.error?.code || event?.code || '';
  console.error(`[llm] responses ${event?.type || 'stream ended'}: ${code || 'no code'}`);
  if (USAGE_SPENT.includes(code)) return new LlmError(Status.PAYMENT_REQUIRED, 'ChatGPT plan usage limit reached');
  if (!event) return new LlmError(null, 'LLM endpoint did not finish its answer');
  return new LlmError(Status.BAD_GATEWAY, `LLM endpoint returned ${event.type}`);
}

/**
 * The completed response in a stream, or the failure that ended it. Each output
 * item arrives whole in its own `response.output_item.done` event; the closing
 * `response.completed` carries the status and usage but, on the plan's endpoint,
 * not always the items again, so they are gathered as they come.
 */
function completedIn(text: string) {
  const events = eventsOf(text);
  const done = events.find((e) => e.type === 'response.completed');
  if (!done) throw failureOf(events.find((e) => ['response.failed', 'response.incomplete', 'error'].includes(e.type)));
  const items = events.filter((e) => e.type === 'response.output_item.done' && e.item).map((e) => e.item);
  const output = items.length ? items : done.response?.output || [];
  if (!output.length)
    console.warn(`[llm] responses stream had no items; events: ${[...new Set(events.map((e) => e.type))].join(', ')}`);
  return { ...done.response, output };
}

/** Rethrows a request that never got (all of) an answer as the shared error. */
const noAnswer = (err: unknown): never => {
  throw unanswered(err);
};

/** POSTs the streamed request; a 30x is refused like every other provider's. */
function post(request: ChatRequest) {
  const headers = { Authorization: `Bearer ${request.apiKey}`, 'Content-Type': 'application/json' };
  const deadline = AbortSignal.timeout(LLM_TIMEOUT_MS);
  const init = {
    method: 'POST',
    redirect: 'error' as const,
    signal: request.signal ? AbortSignal.any([deadline, request.signal]) : deadline,
  };
  return fetch(baseOf(request.baseUrl), { ...init, headers, body: JSON.stringify(bodyFor(request)) }).catch(noAnswer);
}

/** The whole stream's text as the desktop relayed it; a refusal takes the shared error, as a direct one would. */
async function relayed(request: ChatRequest): Promise<string> {
  const answer = await request.transport!(bodyFor(request)).catch(noAnswer);
  if (answer.status < Status.OK || answer.status >= Status.MULTIPLE_CHOICES)
    throw statusError(answer.status, answer.text, answer.retryAfter);
  return answer.text;
}

/** The whole stream's text, once it ends: fetched here, or relayed to where the credential lives. */
async function streamed(request: ChatRequest): Promise<string> {
  if (request.transport) return relayed(request);
  const res = await post(request);
  if (!res.ok) await refuse(res);
  return res.text().catch(noAnswer);
}

/** The Responses API, answering in the shared chat-completions shape. */
export const responses: LlmProvider = {
  name: 'responses',
  handles: isResponses,
  send: async (request) => {
    const response = completedIn(await streamed(request));
    // What the model answered with, by item kind only: no text, no arguments.
    const kinds = (response?.output || []).map((item) => item.type + (item.status ? `:${item.status}` : ''));
    console.log(`[llm] responses ${response?.status || 'completed'}: ${kinds.join(', ') || 'no output'}`);
    return fromResponses(response);
  },
};
