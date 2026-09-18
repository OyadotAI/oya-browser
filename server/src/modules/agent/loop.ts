/**
 * The agentic loop: LLM → tool calls → execute → feed back → repeat until the
 * model answers in text or the iteration limit is reached.
 */
import { BROWSER_TOOLS } from './tools.ts';
import { REQUEST_HUMAN } from './prompt.ts';
import { executeTool } from './executor.ts';
import { recordStep } from './recorder.ts';
import { fill, redact } from './placeholders.ts';
import { trimContext } from './context.ts';
import { chatCompletion } from '../../platform/llm.ts';
import { metrics } from '../../platform/metrics.ts';
import * as usage from '../../platform/usage.ts';
import { DEFAULT_MAX_ITERATIONS, REPEAT_WARNING_AFTER, AGENT_LOG, AGENT_LOG_CHARS } from './constants.ts';

/** Tools after which the page may have changed, so the caller's checkpoint runs. */
const PAGE_CHANGING = new Set(['navigate', 'click', 'press_key']);
/** Arguments whose placeholders are filled with task values before the tool runs. */
const FILLED_ARGS = ['text', 'option', 'url'];

/** Where and how to reach the model. */
export type LlmSettings = {
  /** The credential for the LLM API. */
  openaiKey: string;
  /** The OpenAI-compatible endpoint. */
  baseUrl: string;
  /** The chat model to use. */
  model: string;
};

/** One run's settings and callbacks, shared by every step of the loop. */
export type LoopContext = {
  /** The browser the agent drives. */
  browserId: string;
  /** The calling API key, billed for tokens. */
  apiKey?: string;
  /** Where and how to reach the model. */
  llm: LlmSettings;
  /** Attachable task files by name. */
  files: Record<string, any>;
  /** Task data and secrets, typed through placeholders. */
  values: Record<string, any>;
  /** Values the model must never read. */
  secrets: Record<string, any>;
  /** Told about each tool call before it runs. */
  onToolCall?: (call: any) => void;
  /** Given the final text answer. */
  onText?: (text: string) => void;
  /** Runs after each successful page-changing tool. */
  checkpoint?: () => any;
  /** When given, lets the agent ask a person and wait for the reply. */
  requestHuman?: (ask: any) => any;
  /** The last tool call and how many times in a row it was made. */
  lastCall?: {
    /** The call's name and arguments, as one string. */
    key: string;
    /** How many times in a row it was made. */
    count: number;
  };
};

/** Counts one direction of an iteration's tokens against the key. */
function bill(apiKey, direction, counter, tokens) {
  if (!tokens) return;
  metrics.chatTokens.inc({ direction }, tokens);
  usage.record(apiKey, counter, tokens);
}

/** The tool call's JSON arguments, or {} when they do not parse. */
function parseArgs(raw) {
  let args: Record<string, any> = {};
  try {
    if (raw) args = JSON.parse(raw);
  } catch {}
  return args;
}

/** Arguments with task values filled into their placeholders. */
const filled = (args, values) =>
  Object.fromEntries(Object.entries(args).map(([k, v]) => [k, FILLED_ARGS.includes(k) ? fill(v, values) : v]));

/** Runs a call: request_human asks the person, anything else goes to the browser. */
async function invoke(ctx: LoopContext, name, args) {
  try {
    return name === 'request_human' && ctx.requestHuman
      ? `The person replied: ${await ctx.requestHuman({ reason: 'agent', message: String(args.message || '') })}`
      : await executeTool(ctx.browserId, name, filled(args, ctx.values), ctx.files);
  } catch (err) {
    return `Error: ${err.message}`;
  }
}

/** Records a call that worked, and checkpoints when it may have changed the page. */
async function afterSuccess(ctx: LoopContext, name, args) {
  recordStep(ctx.browserId, name, args, ctx.values);
  if (PAGE_CHANGING.has(name)) await ctx.checkpoint?.();
}

/** Runs one tool call and returns its result, recorded when it worked and redacted for the model. */
async function callTool(ctx: LoopContext, tc) {
  const name = tc.function?.name;
  const args = parseArgs(tc.function?.arguments);
  ctx.onToolCall?.({ name, args });
  const result = await invoke(ctx, name, args);
  if (!String(result).startsWith('Error')) await afterSuccess(ctx, name, args);
  // ponytail: whole secret values are redacted; a filtered piece of one (its digits, a first name) read back from the page is not.
  const seen = redact(result, ctx.secrets) + repeatNote(ctx, name, args);
  if (AGENT_LOG) logCall(ctx, name, args, seen);
  return seen;
}

/** One line per tool call on the server console: what was asked and how the result starts. */
function logCall(ctx: LoopContext, name, args, result) {
  const asked = redact(JSON.stringify(args), ctx.secrets).slice(0, AGENT_LOG_CHARS);
  console.log(
    `[agent] ${ctx.browserId} ${name} ${asked} → ${String(result).replace(/\s+/g, ' ').slice(0, AGENT_LOG_CHARS)}`,
  );
}

/**
 * A model stuck on something that does not work (a field that will not take a
 * value, a button that does nothing) repeats the same call until the iteration
 * limit. From the third identical call in a row, the result tells it so.
 */
function repeatNote(ctx: LoopContext, name, args) {
  const key = name + JSON.stringify(args);
  const count = ctx.lastCall?.key === key ? ctx.lastCall.count + 1 : 1;
  ctx.lastCall = { key, count };
  if (count < REPEAT_WARNING_AFTER) return '';
  return `\n\nNOTE: this is the same ${name} call ${count} times in a row, so it is not working. Try something different (another element, another value format, scrolling, or analyze_page), or stop and explain what is blocking you.`;
}

/** The assistant turn that asked for these calls, as the next request must repeat it. */
const assistantTurn = (msg, toolCalls) => ({
  role: 'assistant',
  content: msg.content ?? null,
  // Spread, not rebuilt: Gemini 3 rejects the next turn unless each call's
  // extra_content (its thought signature) comes back unchanged.
  tool_calls: toolCalls.map((tc) => ({
    ...tc,
    type: 'function',
    function: { name: tc.function?.name, arguments: tc.function?.arguments ?? '{}' },
  })),
});

/** Runs the model's tool calls in order and appends them and their results to the conversation. */
async function handleToolCalls(ctx: LoopContext, allMessages, msg) {
  const toolCalls = msg.tool_calls.filter((tc) => tc && tc.id);
  if (toolCalls.length !== msg.tool_calls.length) console.warn('[chat] Skipped tool calls without id');
  const toolResults = [];
  for (const tc of toolCalls) toolResults.push({ tool_call_id: tc.id, content: await callTool(ctx, tc) });
  allMessages.push(assistantTurn(msg, toolCalls));
  for (const tr of toolResults) allMessages.push({ role: 'tool', tool_call_id: tr.tool_call_id, content: tr.content });
}

/** Asks the model for its next move, billing the tokens. */
async function complete(ctx: LoopContext, allMessages) {
  const { baseUrl, openaiKey, model } = ctx.llm;
  const tools = ctx.requestHuman ? [...BROWSER_TOOLS, REQUEST_HUMAN] : BROWSER_TOOLS;
  const completion = await chatCompletion({ baseUrl, apiKey: openaiKey, model, messages: allMessages, tools });
  // Every iteration of the agentic loop bills, so account per iteration
  // rather than once per request.
  bill(ctx.apiKey, 'input', 'chat_input_tokens', completion.usage?.prompt_tokens || 0);
  bill(ctx.apiKey, 'output', 'chat_output_tokens', completion.usage?.completion_tokens || 0);
  return completion;
}

/** One turn of the loop: the model's final text, or nothing when it called tools or said nothing. */
async function iterate(ctx: LoopContext, allMessages) {
  trimContext(allMessages);
  const choice = (await complete(ctx, allMessages)).choices?.[0];
  if (!choice) throw new Error('No completion in response');
  const msg = choice.message;
  if (msg.tool_calls?.length) return void (await handleToolCalls(ctx, allMessages, msg));
  const text = msg.content?.trim();
  if (text) ctx.onText?.(text);
  return text;
}

/** Loops until the model answers in text, or CHAT_MAX_ITERATIONS runs out. */
export async function agentLoop(ctx: LoopContext, allMessages) {
  const maxIterations = parseInt(process.env.CHAT_MAX_ITERATIONS || DEFAULT_MAX_ITERATIONS, 10);
  for (let iterations = 0; iterations < maxIterations; iterations++) {
    const text = await iterate(ctx, allMessages);
    if (text) return { text, toolCalls: [] };
  }
  return { text: 'Reached iteration limit.', toolCalls: [], limited: true };
}
