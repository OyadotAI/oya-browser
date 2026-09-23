/**
 * The agentic loop: LLM → tool calls → execute → feed back → repeat until the
 * model answers in text or the iteration limit is reached.
 */
import { toolsOn } from './tools.ts';
import { actionsOf } from '../browsers/socket.ts';
import { REMEMBER, REQUEST_HUMAN, UPDATE_PLAN, returnDataTool } from './prompt.ts';
import { notesHere, rememberNote } from './site-notes.ts';
import { executeTool } from './executor.ts';
import { recordStep, elementOf } from './recorder.ts';
import { batchOf, currentId, ELEMENT_MOVED, type Batch } from './batch.ts';
import { fill, redact } from './placeholders.ts';
import { trimContext } from './context.ts';
import { takeScreenshot, addImageTurn } from './screenshot.ts';
import { chatCompletion } from '../../platform/llm.ts';
import { metrics } from '../../platform/metrics.ts';
import * as usage from '../../platform/usage.ts';
import { DEFAULT_MAX_ITERATIONS, AGENT_LOG, AGENT_LOG_CHARS, WRAP_UP_STEPS, STOPPED_TEXT } from './constants.ts';
import { newGuard, afterCall, afterEmpty, failed, type Guard } from './guards.ts';
import { verify, recheck } from './verifier.ts';
import { CHALLENGE_TOOLS, CHALLENGE_HANDLERS, type Challenges } from './challenge-tools.ts';
import { VERIFY_ROUNDS } from './constants.ts';

/** Tools after which the page may have changed, so the caller's checkpoint runs. */
const PAGE_CHANGING = new Set([
  'navigate',
  'click',
  'double_click',
  'press_key',
  'select_option',
  'open_tab',
  'switch_tab',
  'go_back',
  'go_forward',
  'reload',
]);
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
  /** A screenshot taken in this turn, shown to the model after the tool results. */
  image?: string;
  /** The run's loop and empty-reply guard (guards.ts). */
  guard?: Guard;
  /** Set when the guard ends the run: the report it ends on. */
  stop?: string;
  /** Throws when the key's token budget is spent; checked before every model call. */
  budget?: () => void;
  /** The walls the agent may clear itself (CAPTCHA, sign-in, MFA); none offered when absent. */
  challenges?: Challenges;
  /** The agent's plan, as update_plan last wrote it. */
  plan?: any[];
  /** Whether a report of success is checked against the page before it is accepted (verifier.ts). */
  verify?: boolean;
  /** How many times the check sent the report back. */
  rechecks?: number;
  /** The JSON schema of the data the caller wants back; the run then ends with return_data. */
  schema?: Record<string, any>;
  /** The data return_data answered with. */
  data?: any;
  /** Sites whose kept notes this run has shown (site-notes.ts). */
  shownSites?: Set<string>;
  /** Aborted when the caller gives up (the person pressed Stop); the run ends at the next step. */
  signal?: AbortSignal;
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

/** Takes a screenshot, keeping its image for the turn after the tool results. */
async function screenshotFor(ctx: LoopContext) {
  const shot = await takeScreenshot(ctx.browserId, ctx.secrets);
  ctx.image = shot.image;
  return shot.text;
}

/** The plan as the model reads it back: each step, ticked when done. */
function planText(steps: any[] = []) {
  const done = steps.filter((s) => s.done).length;
  const lines = steps.map((s) => `${s.done ? '[x]' : '[ ]'} ${String(s.step ?? '')}`);
  return `Plan (${done} of ${steps.length} done):\n${lines.join('\n')}`;
}

/**
 * Keeps the data the run was asked for and ends the run on it.
 * ponytail: only its presence is checked; the model fills the schema's shape as
 * the tool offered it. A JSON Schema validator is the upgrade if callers see drift.
 */
async function returnData(ctx: LoopContext, args) {
  if (args.data === undefined) return 'Error: pass the answer as data.';
  ctx.data = args.data;
  ctx.stop = `DONE: ${JSON.stringify(args.data)}`;
  return 'Returned.';
}

/**
 * Asks a person: one who can be reached mid-run answers and the run goes on; with
 * none (a chat, where the person reads the reply), the question ends the run as
 * its reply, and their next message answers it.
 */
async function askPerson(ctx: LoopContext, args) {
  const message = String(args.message || '');
  if (ctx.requestHuman) return `The person replied: ${await ctx.requestHuman({ reason: 'agent', message })}`;
  ctx.stop = `NEEDS INPUT: ${message}`;
  return 'Your question goes to the user as your reply; the run stops here until they answer.';
}

/** Tools the loop answers itself, never the browser: the screenshot the model sees, the person, and the plan. */
const LOCAL_TOOLS: Record<string, (ctx: LoopContext, args: any) => Promise<string>> = {
  screenshot: (ctx) => screenshotFor(ctx),
  request_human: askPerson,
  return_data: async (ctx, args) => returnData(ctx, args),
  remember: rememberNote,
  update_plan: async (ctx, args) => ((ctx.plan = Array.isArray(args.steps) ? args.steps : []), planText(ctx.plan)),
};

/** Whether the loop answers `name` itself: a local tool, or a challenge the run was given. */
const isLocal = (ctx: LoopContext, name) =>
  Object.hasOwn(LOCAL_TOOLS, name) || (!!ctx.challenges && Object.hasOwn(CHALLENGE_HANDLERS, name));

/** Runs a tool the loop answers itself. */
const runLocal = (ctx: LoopContext, name, args) =>
  Object.hasOwn(LOCAL_TOOLS, name) ? LOCAL_TOOLS[name](ctx, args) : CHALLENGE_HANDLERS[name](ctx.challenges);

/** The tools offered this run: the ones this browser does, the plan, a person, and the challenges when the run has them. */
const toolsFor = (ctx: LoopContext) => [
  ...toolsOn(actionsOf(ctx.browserId)),
  UPDATE_PLAN,
  REQUEST_HUMAN,
  ...(ctx.challenges ? CHALLENGE_TOOLS : []),
  ...(ctx.schema ? [returnDataTool(ctx.schema)] : []),
  ...(ctx.apiKey ? [REMEMBER] : []),
];

/**
 * Said instead of running a script on a task that holds secrets. redact() only
 * catches a secret written out whole, and a script can hand back a field's value
 * encoded or split, which would reach the model unredacted. Screenshots are
 * withheld from these runs for the same reason (screenshot.ts).
 */
const NO_SCRIPTS =
  'Error: run_script is not available on a task that carries secrets, because a script could read one. Read the page with analyze_page instead.';

/** Runs a call: a local tool here, anything else in the browser. */
async function invoke(ctx: LoopContext, name, args) {
  try {
    if (isLocal(ctx, name)) return await runLocal(ctx, name, args);
    if (name === 'run_script' && Object.keys(ctx.secrets || {}).length) return NO_SCRIPTS;
    return await executeTool(ctx.browserId, name, filled(args, ctx.values), ctx.files);
  } catch (err) {
    return `Error: ${err.message}`;
  }
}

/** Records a call that worked, from the element as it was before it acted, and checkpoints when it may have changed the page. */
async function afterSuccess(ctx: LoopContext, name, args, acted) {
  await recordStep(ctx.browserId, name, args, ctx.values, acted);
  if (PAGE_CHANGING.has(name)) await ctx.checkpoint?.();
}

/** The call's arguments with its element id read in the list the model saw (batch.ts); null when that element is gone. */
function aimedArgs(ctx: LoopContext, tc, batch: Batch) {
  const args = parseArgs(tc.function?.arguments);
  if (args.element_id == null) return args;
  const id = currentId(ctx.browserId, batch, args.element_id);
  return id == null ? null : { ...args, element_id: id };
}

/** Runs the call, unless its element is gone; the element it acted on is taken before it acts. */
async function runAimed(ctx: LoopContext, name, args) {
  if (!args) return { result: ELEMENT_MOVED, acted: undefined };
  const acted = args.element_id == null ? undefined : elementOf(ctx.browserId, args.element_id);
  return { result: await invoke(ctx, name, args), acted };
}

/** Runs one tool call and returns its result, recorded when it worked and redacted for the model. */
async function callTool(ctx: LoopContext, tc, batch: Batch) {
  const name = tc.function?.name;
  const args = aimedArgs(ctx, tc, batch);
  ctx.onToolCall?.({ name, args: args || parseArgs(tc.function?.arguments) });
  const { result, acted } = await runAimed(ctx, name, args);
  if (!String(result).startsWith('Error')) await afterSuccess(ctx, name, args, acted);
  const seen = guarded(ctx, name, args, String(result));
  if (AGENT_LOG) logCall(ctx, name, args, seen);
  return seen;
}

/** The result as the model reads it: redacted, with the loop guard's note; a run the guard stops is marked. */
function guarded(ctx: LoopContext, name, args, result: string) {
  const { note, stop } = afterCall(ctx.guard, name, args, result);
  if (stop) ctx.stop = stop;
  // ponytail: whole secret values are redacted; a filtered piece of one (its digits, a first name) read back from the page is not.
  return redact(result, ctx.secrets) + note + notesHere(ctx);
}

/** One line per tool call on the server console: what was asked and how the result starts. */
function logCall(ctx: LoopContext, name, args, result) {
  const asked = redact(JSON.stringify(args), ctx.secrets).slice(0, AGENT_LOG_CHARS);
  console.log(
    `[agent] ${ctx.browserId} ${name} ${asked} → ${String(result).replace(/\s+/g, ' ').slice(0, AGENT_LOG_CHARS)}`,
  );
}

/** The assistant turn that asked for these calls, as the next request must repeat it. */
const assistantTurn = (msg, toolCalls) => ({
  role: 'assistant',
  content: msg.content ?? null,
  // What the provider needs repeated next turn (Claude's thinking blocks), unchanged.
  ...(msg.extra_content ? { extra_content: msg.extra_content } : {}),
  // Spread, not rebuilt: Gemini 3 rejects the next turn unless each call's
  // extra_content (its thought signature) comes back unchanged.
  tool_calls: toolCalls.map((tc) => ({
    ...tc,
    type: 'function',
    function: { name: tc.function?.name, arguments: tc.function?.arguments ?? '{}' },
  })),
});

/** Runs one turn's calls in order, each aimed at the element list the model saw. */
async function runCalls(ctx: LoopContext, toolCalls) {
  const results = [];
  const batch = batchOf(ctx.browserId);
  for (const tc of toolCalls) results.push({ tool_call_id: tc.id, content: await callTool(ctx, tc, batch) });
  return results;
}

/** Runs the model's tool calls in order and appends them and their results to the conversation. */
async function handleToolCalls(ctx: LoopContext, allMessages, msg) {
  const toolCalls = msg.tool_calls.filter((tc) => tc && tc.id);
  if (toolCalls.length !== msg.tool_calls.length) console.warn('[chat] Skipped tool calls without id');
  const toolResults = await runCalls(ctx, toolCalls);
  allMessages.push(assistantTurn(msg, toolCalls));
  for (const tr of toolResults) allMessages.push({ role: 'tool', tool_call_id: tr.tool_call_id, content: tr.content });
  if (ctx.image) addImageTurn(allMessages, ctx.image);
  ctx.image = undefined;
}

/** Asks the model for its next move, billing the tokens. */
async function complete(ctx: LoopContext, allMessages) {
  const { baseUrl, openaiKey, model } = ctx.llm;
  ctx.budget?.();
  const tools = toolsFor(ctx);
  const completion = await chatCompletion({ baseUrl, apiKey: openaiKey, model, messages: allMessages, tools });
  // Every iteration of the agentic loop bills, so account per iteration
  // rather than once per request.
  billUsage(ctx, completion.usage);
  return completion;
}

/** Counts a model call's tokens against the key. */
function billUsage(ctx: LoopContext, used) {
  bill(ctx.apiKey, 'input', 'chat_input_tokens', used?.prompt_tokens || 0);
  bill(ctx.apiKey, 'output', 'chat_output_tokens', used?.completion_tokens || 0);
}

/**
 * Whether the report stands: failures and runs that did nothing are not checked;
 * a report of success that the check finds unsupported is sent back with the
 * reason, at most VERIFY_ROUNDS times.
 */
async function accepted(ctx: LoopContext, allMessages, text: string) {
  if (!ctx.verify || failed(text) || !ctx.guard?.total || (ctx.rechecks || 0) >= VERIFY_ROUNDS) return true;
  const verdict = await verify(ctx, allMessages, text, (used) => billUsage(ctx, used));
  if (verdict.pass) return true;
  ctx.rechecks = (ctx.rechecks || 0) + 1;
  allMessages.push({ role: 'assistant', content: text }, { role: 'user', content: recheck(verdict.reason) });
  return false;
}

/** One turn of the loop: the model's final text (or the guard's), or nothing when it called tools. */
async function iterate(ctx: LoopContext, allMessages) {
  trimContext(allMessages);
  const choice = (await complete(ctx, allMessages)).choices?.[0];
  if (!choice) throw new Error('No completion in response');
  const msg = choice.message;
  if (msg.tool_calls?.length) return (await handleToolCalls(ctx, allMessages, msg), ctx.stop);
  return answerOf(ctx, allMessages, msg);
}

/** A reply without tool calls: the final answer once it is accepted, else nothing (the loop goes on). */
async function answerOf(ctx: LoopContext, allMessages, msg) {
  const text = msg.content?.trim() || emptyReply(ctx, allMessages);
  if (!text || !(await accepted(ctx, allMessages, text))) return undefined;
  ctx.onText?.(text);
  return text;
}

/** An empty reply: a nudge to act or finish is added and nothing is answered, until the guard ends the run. */
function emptyReply(ctx: LoopContext, allMessages) {
  const { nudge, stop } = afterEmpty(ctx.guard);
  if (nudge) allMessages.push({ role: 'user', content: nudge });
  return stop;
}

/** How a run ended. */
export type AgentResult = {
  /** The final report, or the guard's. */
  text: string;
  /** Kept for callers that read it; the calls go to onToolCall. */
  toolCalls: any[];
  /** Whether the report says the task could not be done. */
  failed: boolean;
  /** Set when the run ran out of iterations. */
  limited?: boolean;
  /** What return_data answered, for a run given a schema. */
  data?: any;
};

/**
 * A run cut off at its limit answers nothing at all, which is the worst of both:
 * the work is paid for and what it found is thrown away. A few steps from the
 * end the agent is told to stop and report, so the answer is at least partial.
 */
function wrapUp(allMessages, left: number, maxIterations: number) {
  if (left !== WRAP_UP_STEPS || maxIterations <= WRAP_UP_STEPS) return;
  allMessages.push({
    role: 'user',
    content: `You have ${WRAP_UP_STEPS} steps left in this run. Stop exploring and reply now with your report: DONE: and the answer if you have it, or FAILED: and what you found and what blocked you.`,
  });
}

/** Loops until the model answers in text, or CHAT_MAX_ITERATIONS runs out. */
export async function agentLoop(ctx: LoopContext, allMessages): Promise<AgentResult> {
  const maxIterations = parseInt(process.env.CHAT_MAX_ITERATIONS || DEFAULT_MAX_ITERATIONS, 10);
  ctx.guard ??= newGuard();
  for (let iterations = 0; iterations < maxIterations; iterations++) {
    wrapUp(allMessages, maxIterations - iterations, maxIterations);
    const text = ctx.signal?.aborted ? STOPPED_TEXT : await iterate(ctx, allMessages);
    if (text) return { text, toolCalls: [], failed: failed(text), ...(ctx.data !== undefined && { data: ctx.data }) };
  }
  return { text: 'Reached iteration limit.', toolCalls: [], limited: true, failed: true };
}
