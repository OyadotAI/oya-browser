/**
 * Chat service, LLM + MCP-style tool execution for browser control. This is the
 * agent module's entry point; the loop, tools, recording and placeholders live
 * in the files beside it.
 */

import * as keyConfig from '../config/service.ts';
import { metrics } from '../../platform/metrics.ts';
import { checkHourly } from '../../platform/limits.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { redact, isFileValue } from './placeholders.ts';
import { startRun } from './recorder.ts';
import { systemPrompt } from './prompt.ts';
import { agentLoop } from './loop.ts';

export { FILTERS, PLACEHOLDER, pipesOf, fill, redact, isFileValue, dataKey } from './placeholders.ts';
export { selectOptionIn, UPLOAD_FILE_JS, uploadFileIn } from './page-scripts.ts';
export { lastRun } from './recorder.ts';
export { elementIndex, analysisText, elementList, pageGuide } from './element-index.ts';
export { PAGE_FORMAT } from './constants.ts';

/**
 * A runaway agent loop is the most expensive thing this control plane can do
 * on someone else's behalf, so the ceiling is checked before the first call.
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

/**
 * The model settings for a run. Settings belong to the calling API key; a key
 * that has set none falls back to the deployment-wide values.
 */
function llmFor(apiKey) {
  const { openaiKey, baseUrl, model, own } = keyConfig.resolve(apiKey);
  enforceBudget(apiKey, own);
  if (!openaiKey) {
    throw new Error(
      'No LLM key configured for this API key. Add one in Settings, run `oya init`, or POST /api/config.',
    );
  }
  return { openaiKey, baseUrl, model };
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

/** A fresh recorded run for the task, its prompt redacted to placeholders. */
function newRun(messages, values, secrets) {
  const prompt = messages.filter((m) => m.role === 'user').at(-1)?.content;
  return {
    prompt: typeof prompt === 'string' ? redact(prompt, values) : '',
    steps: [],
    elements: [],
    secrets: Object.keys(secrets),
  };
}

/**
 * Run the agentic loop: LLM → tool calls → execute → feed back → repeat until done.
 * Streams the final text response.
 *
 * `data` values are typed through `{{key}}` placeholders and redacted from everything
 * the model reads. `checkpoint` runs after page-changing tools; `requestHuman`, when
 * given, lets the agent ask a person and wait.
 */
export async function runChat(browserId, messages, options: any = {}) {
  const { apiKey, data = {}, secrets = {} } = options;
  const llm = llmFor(apiKey);
  const { files, scalars, values } = taskValues(data, secrets);
  await startRun(browserId, newRun(messages, values, secrets));
  const system = { role: 'system', content: systemPrompt(values, scalars, files, secrets) };
  const allMessages = [system, ...messages.map((m) => ({ ...m, content: redact(m.content, secrets) }))];
  return agentLoop({ ...options, browserId, llm, files, values, secrets }, allMessages);
}
