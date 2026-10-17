/**
 * Chat service, LLM + MCP-style tool execution for browser control. This is the
 * agent module's entry point; the loop, tools, recording and placeholders live
 * in the files beside it.
 */

import { randomUUID } from 'node:crypto';
import * as keyConfig from '../config/service.ts';
import { resolveFresh } from '../config/chatgpt.ts';
import { agentKeyUnclaimed } from '../auth/service.ts';
import { metrics } from '../../platform/metrics.ts';
import { checkHourly } from '../../platform/limits.ts';
import { HttpError } from '../../platform/errors.ts';
import { LlmError } from '../../platform/llm/index.ts';
import { Status } from '../../platform/http-status.ts';
import { redact, isFileValue } from './placeholders.ts';
import { startRun, lastRun, markMessage } from './recorder.ts';
import { forgetPage } from './changes.ts';
import { AGENT_VERIFY } from './constants.ts';
import { systemPrompt, FOLLOW_UP_NOTE, todayLine } from './prompt.ts';
import { agentLoop, type AgentResult } from './loop.ts';
import { fenced, startFindings, statusOf, type KeptRun, type Started } from './findings.ts';
import { requireLlmAllowed } from './llm-policy.ts';
import { container } from '../../app/container.ts';

export { FILTERS, PLACEHOLDER, pipesOf, fill, redact, isFileValue, dataKey } from './placeholders.ts';
export { selectOptionIn, UPLOAD_FILE_JS, uploadFileIn } from './page-scripts.ts';
export { lastRun, hasReplayableSteps } from './recorder.ts';
export { elementIndex, analysisText, elementList, pageGuide } from './element-index.ts';
export { PAGE_FORMAT } from './constants.ts';
export { NEEDS_INPUT } from './loop.ts';
export { requireLlmAllowed, allowedLlm } from './llm-policy.ts';
export { executeTool } from './executor.ts';
export { LIBRARY_TOOL_NAMES } from './library-tools.ts';
export { BROWSER_TOOLS, toolsOn } from './tools.ts';
export { CHALLENGE_TOOLS, CHALLENGE_HANDLERS } from './challenge-tools.ts';

/**
 * A runaway agent loop is the most expensive thing this control plane can do
 * on someone else's behalf, so the ceiling is checked before every model call,
 * not only the first: one long run could otherwise spend far past it.
 * A key with its own LLM credential pays for its own tokens and has no ceiling.
 */
function enforceBudget(apiKey, own) {
  const budget: any = own ? { allowed: true } : checkHourly('chatTokensPerHour', apiKey);
  if (budget.allowed) return;
  metrics.chatRequests.inc({ outcome: 'quota' });
  throw new HttpError(
    Status.TOO_MANY_REQUESTS,
    `Chat token quota reached for this hour (${budget.current}/${budget.quota})`,
  );
}

/** What a key with no model is told: what to configure and the three ways to do it. */
const noLlm = () =>
  new HttpError(
    Status.UNPROCESSABLE,
    "No LLM key configured for this project. Add yours in the desktop app's Ask panel or in Settings > AI model, run `oya init`, or POST /api/config.",
    { code: 'llm_unconfigured' },
  );

/** What an agent's own key is told: it brings its own model, and how to set one with the key it already has. */
const bringYourOwn = () =>
  new HttpError(
    Status.UNPROCESSABLE,
    `Agent keys bring their own LLM: POST /api/config {"llm_provider": ${keyConfig.llmProviderList()}, "openai_api_key": "<your provider key>"} (the field takes any provider's key; chat_model optional), then retry.`,
    { code: 'llm_bring_your_own' },
  );

/** The refusal for a key with no model: an agent's own key is told to bring one, anyone else where to set it. */
const missingLlm = (apiKey) => (agentKeyUnclaimed(apiKey) ? bringYourOwn() : noLlm());

/** Refuses a chat before it starts when the key has no model, so the caller gets a real status, not a 200 with an error inside. */
export function requireLlm(apiKey) {
  if (!keyConfig.resolve(apiKey).openaiKey) throw missingLlm(apiKey);
}

/**
 * The model settings for a run. Settings belong to the calling API key; a key
 * that has set none falls back to the deployment-wide values.
 */
async function llmFor(apiKey, browserId?: string) {
  const { openaiKey, baseUrl, model, own, transport } = await resolveFresh(apiKey, browserId);
  enforceBudget(apiKey, own);
  if (!openaiKey) throw missingLlm(apiKey);
  const freshKey = () => resolveFresh(apiKey, browserId).then((fresh) => fresh.openaiKey);
  const llm = { openaiKey, baseUrl, model, hosted: !own, freshKey, transport };
  return { llm, budget: () => stepAllowed(apiKey, own) };
}

/**
 * Checked before every model call, not only the first: the hourly token ceiling,
 * and the person's plan, whose allowance a long run or runs side by side could
 * otherwise spend far past.
 */
async function stepAllowed(apiKey, own) {
  enforceBudget(apiKey, own);
  await container.billing.entitlements.admitAgent(apiKey);
}

/** The run's model settings, once the project's model policy and the person's plan admit another agent run. */
async function admitted(apiKey, browserId?: string) {
  const settings = await llmFor(apiKey, browserId);
  await requireLlmAllowed(apiKey, settings.llm.baseUrl);
  await settings.budget();
  return settings;
}

/**
 * The task's data split up. A file is attached, never typed, so it is split out
 * before anything that fills or redacts a placeholder sees it, String(a file) is
 * "[object Object]". `data` the model reads; `secrets` it never does. Both are
 * typed through placeholders.
 */
function taskValues(data, secrets) {
  const files: Record<string, any> = Object.fromEntries(Object.entries(data).filter(([, v]) => isFileValue(v)));
  const scalars = Object.fromEntries(Object.entries(data).filter(([, v]) => !isFileValue(v)));
  return { files, scalars, values: { ...scalars, ...secrets } };
}

/** What the person asked, every message of the chat, redacted to placeholders: a playbook is the whole conversation's task. */
function chatPrompt(messages, values) {
  const asked = messages.filter((m) => m.role === 'user' && typeof m.content === 'string').map((m) => m.content);
  return redact(asked.join('\n\n'), values);
}

/** A fresh recorded run for the task, its prompt redacted to placeholders. */
function newRun(messages, values, secrets) {
  return { prompt: chatPrompt(messages, values), steps: [], elements: [], secrets: Object.keys(secrets) };
}

/** The person's messages, oldest first and redacted, for the check before done. */
const askedIn = (messages, secrets) =>
  messages.filter((m) => m.role === 'user' && typeof m.content === 'string').map((m) => redact(m.content, secrets));

/** Whether the report is checked before done, and the person's messages it is checked against. */
const checkFor = (options, messages, secrets) => ({
  verify: options.verify ?? AGENT_VERIFY,
  asked: askedIn(messages, secrets),
});

/** Whether this is a follow-up in a chat already under way. */
const followUp = (messages) => messages.filter((m) => m.role === 'user').length > 1;

/**
 * Starts recording the task, or keeps recording it. A follow-up message ("now fill
 * the form") carries on the same chat, so its steps join the run the first message
 * started. Starting over on every message saved a playbook of the last message alone.
 * ponytail: a playbook replayed on the same browser between two messages becomes the run followed; a chat id on the request if that bites.
 */
async function begin(browserId, messages, values, secrets) {
  const run = lastRun(browserId);
  if (followUp(messages) && run) {
    run.prompt = chatPrompt(messages, values);
    run.secrets = [...new Set([...run.secrets, ...Object.keys(secrets)])];
    markMessage(browserId);
  } else await startRun(browserId, newRun(messages, values, secrets), values);
  forgetPage(browserId);
}

/** The system prompt, with the follow-up note when the chat is already under way. */
function systemFor(messages, values, scalars, files, secrets) {
  const note = followUp(messages) ? `\n\n${FOLLOW_UP_NOTE}` : '';
  return `${systemPrompt(values, scalars, files, secrets)}\n\n${todayLine()}${note}`;
}

/**
 * Run the agentic loop: LLM → tool calls → execute → feed back → repeat until done.
 * Answers the final text, and whether it reports a failure.
 *
 * `data` values are typed through `{{key}}` placeholders and redacted from everything
 * the model reads. `checkpoint` runs after page-changing tools; `requestHuman`, when
 * given, lets the agent ask a person and wait.
 */
export async function runChat(browserId, messages, options: any = {}) {
  const { apiKey, data = {}, secrets = {} } = options;
  const { llm, budget } = await admitted(apiKey, browserId);
  const task = taskValues(data, secrets);
  await begin(browserId, messages, task.values, secrets);
  const { memory, allMessages } = prepare(browserId, messages, options, task, secrets);
  const run = { ...options, browserId, llm, budget, ...checkFor(options, messages, secrets), ...task, secrets };
  const work = () => agentLoop({ ...run, ...keeping(memory) }, allMessages);
  return settled(work, memory?.kept, options.signal).catch(rejectedProvider);
}

/** The run's start: what it keeps for the chat's next message, and the conversation it opens with. */
function prepare(browserId, messages, options, { files, scalars, values }, secrets) {
  const memory = remembering(browserId, messages, options);
  const system = systemFor(messages, values, scalars, files, secrets);
  return { memory, allMessages: withDigest(opening(system, messages, secrets), memory, secrets) };
}

/** What the model starts from: the system prompt, then the chat with its secrets redacted. */
const opening = (system: string, messages, secrets) => [
  { role: 'system', content: system },
  ...messages.map((m) => ({ ...m, content: redact(m.content, secrets) })),
];

/** The loop's listener that keeps each result for the next message, when the run is remembered. */
const keeping = (memory: Started | null) => (memory ? { onToolResult: memory.kept.keep.bind(memory.kept) } : {});

/**
 * Keeps what the run reads for the chat's next message, when the caller named the chat's lane (the desktop
 * names one per tab). A lane this server names is new on every request, so nothing could ever read it back.
 */
const remembering = (browserId, messages, options): Started | null =>
  (options.remember && startFindings(browserId, followUp(messages), !!Object.keys(options.secrets ?? {}).length)) ||
  null;

/**
 * The conversation with what earlier runs read on the assistant's side, just before the newest message: page
 * text never sits in the person's message or the system prompt. Not as a made-up tool call: a provider can
 * refuse one it did not make (Gemini wants a thought signature on every function call). Redacted with this
 * message's secrets too (one given only now was not redacted when read). New objects, so the chat itself (the
 * recorded prompt, the check before done) never sees it.
 */
function withDigest(allMessages: any[], memory: Started | null, secrets) {
  const at = allMessages.length - 1;
  if (!memory?.digest || allMessages[at]?.role !== 'user') return allMessages;
  const digest = redact(fenced(memory.digest, randomUUID()), secrets);
  const before = allMessages[at - 1];
  if (before?.role === 'assistant' && typeof before.content === 'string' && !before.tool_calls)
    return allMessages.with(at - 1, { ...before, content: `${before.content}\n\n${digest}` });
  return allMessages.toSpliced(at, 0, { role: 'assistant', content: digest });
}

/** Runs the loop and records how it ended for the next message: a Stop at once, since the run finishes its model call first. */
function settled(work: () => Promise<AgentResult>, kept: KeptRun | undefined, signal?: AbortSignal) {
  if (!kept) return work();
  signal?.addEventListener('abort', () => kept.settle('stopped'), { once: true });
  return work().then(
    (result) => (kept.settle(statusOf(result)), result),
    (error) => (kept.settle(statusOf(undefined, error)), Promise.reject(error)),
  );
}

/** Provider answers that mean the key or model is wrong, not that the provider is down. */
const KEY_PROBLEMS: number[] = [Status.BAD_REQUEST, Status.UNAUTHORIZED, Status.FORBIDDEN, Status.NOT_FOUND];

/**
 * Provider billing failures get a distinct actionable code; key/model refusals reopen
 * Ask's key card rather than becoming anonymous 500s. The provider's own words stay
 * in the log: the base URL is tenant-set, so echoing them would be a read primitive.
 */
function rejectedProvider(err): never {
  if (err instanceof LlmError && err.status === Status.PAYMENT_REQUIRED) throw providerPaymentRequired();
  if (err instanceof LlmError && !KEY_PROBLEMS.includes(err.status as number)) throw modelUnavailable();
  if (!(err instanceof LlmError) || !KEY_PROBLEMS.includes(err.status as number)) throw err;
  throw new HttpError(
    Status.UNPROCESSABLE,
    `Your AI provider refused the request (${err.status}). Check the API key and model under Ask > Model.`,
    { code: 'llm_rejected' },
  );
}

/** The model kept failing (no answer, an overload, a dropped stream) even after the retries: say so, not "server error". */
const modelUnavailable = () =>
  new HttpError(Status.BAD_GATEWAY, 'The AI model did not answer after a few tries. Send it again in a moment.', {
    code: 'llm_unavailable',
  });

/** Explain exhausted upstream credit without exposing provider response bodies or implying an Oya subscription failure. */
function providerPaymentRequired() {
  return new HttpError(
    Status.PAYMENT_REQUIRED,
    'The AI provider requires payment or more credits for this request. Check its billing or spending limit; if you use a shared provider, contact your administrator.',
    { code: 'llm_payment_required' },
  );
}
