/**
 * Free-text answers written by the model at replay time. A playbook replays without
 * the model, except for the one or two fields a person would write fresh each time: a
 * comment, the reason for a request, the answer to a question. Recorded as they were
 * typed, those replayed the first run's words into every later case.
 */
import { chatCompletion } from '../../platform/llm.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import * as keyConfig from '../config/service.ts';
import { fill, requireLlmAllowed } from '../agent/chat.ts';
import { MAX_ANSWER_CHARS } from './constants.ts';
import { container } from '../../app/container.ts';
import { llmCost } from '../billing/index.ts';
import * as usage from '../../platform/usage.ts';

/** What the model is told. */
const WRITER = `You fill in one free-text field of a web form for a person, as part of a task they described.
Write only the text that goes in the field: no quotes, no preamble, no labels. Base it on the task and its values; never invent facts the task does not give. Keep it as short as the field needs, in the language the task is in.`;

/** The model's question: the task, this run's values, the field, and how it was answered before. */
function brief(question: string, task = '', values: Record<string, unknown> = {}, example = '') {
  const shown = Object.entries(values).filter(([, v]) => typeof v === 'string' || typeof v === 'number');
  const lines = [`TASK:\n${fill(task, values) || '(none given)'}`];
  if (shown.length) lines.push(`VALUES:\n${shown.map(([k, v]) => `${k}: ${v}`).join('\n')}`);
  lines.push(`FIELD:\n${question}`);
  if (example) lines.push(`AN EARLIER RUN WROTE (for its own data, as a guide to length and tone only):\n${example}`);
  return lines.join('\n\n');
}

/** The key's model, or a refusal a caller can act on. */
function modelFor(apiKey) {
  const { openaiKey, baseUrl, model } = keyConfig.resolve(apiKey);
  if (!openaiKey) throw new HttpError(Status.UNPROCESSABLE, 'No LLM key configured to answer this field.');
  return { apiKey: openaiKey, baseUrl, model };
}

/** The key's model, once its project's policy allows page content to go to it. */
async function allowedModel(apiKey) {
  const model = modelFor(apiKey);
  await requireLlmAllowed(apiKey, model.baseUrl);
  return model;
}

/** One answer is one agent step, and on the operator's model its cost is the person's too. */
function bill(apiKey, used) {
  usage.record(apiKey, 'agent_steps', 1);
  const { own, model } = keyConfig.resolve(apiKey);
  if (!own) usage.record(apiKey, 'hosted_llm_microusd', llmCost(model, used));
}

/**
 * The text for one free-text field, from the key's own model; the person's plan
 * admits it and is billed for it like any agent step.
 * ponytail: not counted against the hourly chat budget; one short call per field.
 */
export async function answerField(apiKey, question: string, context: any = {}) {
  const model = await allowedModel(apiKey);
  await container.billing.entitlements.admitAgent(apiKey);
  const asked = brief(question, context.task, context.values, context.example);
  const reply = await chatCompletion({ ...model, messages: conversation(asked) });
  bill(apiKey, reply.usage);
  return String(reply.choices?.[0]?.message?.content ?? '')
    .trim()
    .slice(0, MAX_ANSWER_CHARS);
}

/** The writer's instructions and the brief, as the model reads them. */
const conversation = (asked: string) => [
  { role: 'system', content: WRITER },
  { role: 'user', content: asked },
];
