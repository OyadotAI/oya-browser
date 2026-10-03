/**
 * Chat service, LLM + MCP-style tool execution for browser control. This is the
 * agent module's entry point; the loop, tools, recording and placeholders live
 * in the files beside it.
 */

import * as keyConfig from '../config/service.ts';
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
import { systemPrompt, FOLLOW_UP_NOTE } from './prompt.ts';
import { agentLoop } from './loop.ts';
import { container } from '../../app/container.ts';

export { FILTERS, PLACEHOLDER, pipesOf, fill, redact, isFileValue, dataKey } from './placeholders.ts';
export { selectOptionIn, UPLOAD_FILE_JS, uploadFileIn } from './page-scripts.ts';
export { lastRun, hasReplayableSteps } from './recorder.ts';
export { elementIndex, analysisText, elementList, pageGuide } from './element-index.ts';
export { PAGE_FORMAT } from './constants.ts';
export { NEEDS_INPUT } from './loop.ts';
export { executeTool } from './executor.ts';
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
function llmFor(apiKey) {
  const { openaiKey, baseUrl, model, own } = keyConfig.resolve(apiKey);
  enforceBudget(apiKey, own);
  if (!openaiKey) throw missingLlm(apiKey);
  return { llm: { openaiKey, baseUrl, model, hosted: !own }, budget: () => stepAllowed(apiKey, own) };
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

/** The run's model settings, once the person's plan admits another agent run. */
async function admitted(apiKey) {
  const settings = llmFor(apiKey);
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
  } else await startRun(browserId, newRun(messages, values, secrets));
  forgetPage(browserId);
}

/** The system prompt, with the follow-up note when the chat is already under way. */
function systemFor(messages, values, scalars, files, secrets) {
  const note = followUp(messages) ? `\n\n${FOLLOW_UP_NOTE}` : '';
  return systemPrompt(values, scalars, files, secrets) + note;
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
  const { llm, budget } = await admitted(apiKey);
  const { files, scalars, values } = taskValues(data, secrets);
  await begin(browserId, messages, values, secrets);
  const system = { role: 'system', content: systemFor(messages, values, scalars, files, secrets) };
  const allMessages = [system, ...messages.map((m) => ({ ...m, content: redact(m.content, secrets) }))];
  const run = { ...options, browserId, llm, budget, ...checkFor(options, messages, secrets), files, values, secrets };
  return agentLoop(run, allMessages).catch(rejectedKey);
}

/** Provider answers that mean the key or model is wrong, not that the provider is down. */
const KEY_PROBLEMS: number[] = [Status.BAD_REQUEST, Status.UNAUTHORIZED, Status.FORBIDDEN, Status.NOT_FOUND];

/**
 * A provider that refused the key or model becomes an error the caller can act on (Ask
 * reopens its key card on the code), not an anonymous 500. The provider's own words stay
 * in the log: the base URL is tenant-set, so echoing them would be a read primitive.
 */
function rejectedKey(err): never {
  if (!(err instanceof LlmError) || !KEY_PROBLEMS.includes(err.status as number)) throw err;
  throw new HttpError(
    Status.UNPROCESSABLE,
    `Your AI provider refused the request (${err.status}). Check the API key and model under Ask > Model.`,
    { code: 'llm_rejected' },
  );
}
