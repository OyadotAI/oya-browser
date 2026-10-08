/**
 * Routes that drive a browser: one action, or a natural-language chat with
 * the browser tools.
 */
import { sendCommand } from '../socket.ts';
import { runChat, lastRun, hasReplayableSteps, requireLlm, NEEDS_INPUT } from '../../agent/chat.ts';
import { control } from '../../control/service.ts';
import * as usage from '../../../platform/usage.ts';
import { Status } from '../../../platform/http-status.ts';
import { HttpError, answerFor, sendError } from '../../../platform/errors.ts';
import * as flow from '../../playbooks/flow-recorder.ts';
import { getKey, longJson, validData } from '../../../app/http.ts';
import { noTimeouts, refuseAction } from './helpers.ts';
import { CHAT_DRAIN_MS, MAX_SCHEMA_CHARS } from '../constants.ts';
import { challengesFor, quietCheckpointFor } from '../../playbooks/checkpoint.ts';
import { LiveRun } from '../../agent/run-events.ts';

/** Runs one action on a browser; server-internal actions are refused. */
export async function runCommand(req, res) {
  // Navigate can take up to 90s, disable socket timeout for this request
  noTimeouts(req, res);
  const { action, params } = req.body;
  if (refuseAction(res, action)) return;
  try {
    res.json(await commandResult(req, req.params.browserId, action, params));
  } catch (err) {
    commandFailed(req, res, err);
  }
}

/** Sends the command and books it. */
async function commandResult(req, browserId, action, params) {
  const result = await sendCommand(browserId, action, params || {});
  // A recording in progress keeps the navigations the person asked for here; what
  // they click and type is seen in the page itself.
  if (result?.ok !== false) flow.noteCommand(browserId, action, params || {});
  usage.record(getKey(req), 'commands');
  if (result?.ok === false) usage.record(getKey(req), 'command_errors');
  return result;
}

/**
 * A command that threw: counted as an error. An HttpError answers itself, with
 * `ok: false` so a command's failure reads the same whatever its status. Anything
 * else is a bug, and goes to the API's handler for the generic 500 and its reference.
 */
function commandFailed(req, res, err) {
  usage.record(getKey(req), 'command_errors');
  if (!(err instanceof HttpError)) throw err;
  const { body } = answerFor(err, req);
  res.status(err.status).json({ ok: false, ...body });
}

/** Whether the key has a model to chat with; answers the refusal itself when it does not. */
function withLlm(req, res) {
  try {
    requireLlm(getKey(req));
    return true;
  } catch (err) {
    sendError(res, err, req);
    return false;
  }
}

/** Why a chat's data or secrets were refused. */
const BAD_DATA = 'data must map names to strings, numbers or a file() value; secrets takes strings and numbers only';

/** Why a chat's schema was refused. */
const BAD_SCHEMA = `schema must be a JSON schema object of at most ${MAX_SCHEMA_CHARS} characters`;

/** Whether a schema for the answer is absent, or a JSON object of a sensible size. */
const validSchema = (schema) =>
  schema === undefined ||
  (!!schema &&
    typeof schema === 'object' &&
    !Array.isArray(schema) &&
    JSON.stringify(schema).length <= MAX_SCHEMA_CHARS);

/** Why a chat's body cannot run, or null when it can. */
function badChat({ messages, data = {}, secrets = {}, schema }) {
  if (!messages || !Array.isArray(messages)) return 'messages array required';
  if (!validData(data) || !validData(secrets, { files: false })) return BAD_DATA;
  return validSchema(schema) ? null : BAD_SCHEMA;
}

/** LLM + MCP tools for natural-language browser control; with a schema, the answer comes back as data in that shape. */
export async function chat(req, res) {
  noTimeouts(req, res);
  const bad = badChat(req.body);
  if (bad) return res.status(Status.BAD_REQUEST).json({ error: bad });
  // Checked before the 200 goes out: with no model there is nothing to converse with, and the caller deserves the real status.
  if (!withLlm(req, res)) return;
  const { messages, data = {}, secrets = {}, schema } = req.body;
  const signal = abortOnHangUp(res);
  await inFlight(longJson(res, () => converse(req, messages, { data, secrets, schema, signal })));
}

/** Chats this replica is answering, so a shutdown lets them finish instead of cutting them off mid-run. */
const chats = new Set<Promise<unknown>>();

/** Holds a chat in the in-flight set until it settles. */
function inFlight(work: Promise<unknown>) {
  chats.add(work);
  return work.finally(() => chats.delete(work));
}

/**
 * Waits for the chats in flight, for shutdown: a deploy otherwise killed the
 * agent mid-task and the person saw "terminated". Gives up after `ms`, so a
 * stuck run cannot hold the process past its grace period.
 */
export function drainChats(ms = CHAT_DRAIN_MS) {
  if (!chats.size) return Promise.resolve();
  console.log(`[oya] Waiting for ${chats.size} chat(s) to finish before exiting`);
  let timer;
  const deadline = new Promise((resolve) => (timer = setTimeout(resolve, ms)));
  return Promise.race([Promise.allSettled([...chats]), deadline]).finally(() => clearTimeout(timer));
}

/** A signal aborted when the caller hangs up before the answer (the desktop's Stop), so the run stops too. */
function abortOnHangUp(res) {
  const ac = new AbortController();
  res.on('close', () => res.writableEnded || ac.abort());
  return ac.signal;
}

/** The walls a chat's agent may clear itself (CAPTCHA, sign-in, MFA), and the automatic tries between steps. */
const walls = (key, browserId) => ({
  challenges: challengesFor(key, browserId),
  checkpoint: quietCheckpointFor(key, browserId),
});

/**
 * Puts one moment of a chat run on the project's event log, so webhooks and the
 * Slack sink see the chat's whole lifecycle like a background run's. Never on
 * the critical path: the reply goes out regardless.
 */
function announceChat(key, browserId, live: LiveRun, type: string, detail: object = {}) {
  control()
    .emit(key, type, browserId, { runId: live.runId, source: 'chat', ...detail })
    .catch((err) => console.error(`[chat] ${type} not recorded:`, err.message));
}

/**
 * How a chat's answer ends its run: stopped to ask a person (`run.needs_attention`,
 * answered in the chat, not by resuming), failed, or completed.
 */
function announceEnd(key, browserId, live: LiveRun, result) {
  if (result.text.startsWith(NEEDS_INPUT))
    return announceChat(key, browserId, live, 'run.needs_attention', {
      reason: 'agent',
      message: live.safe(result.text.slice(NEEDS_INPUT.length)),
    });
  if (result.failed) return announceChat(key, browserId, live, 'run.failed', { error: live.safe(result.text) });
  announceChat(key, browserId, live, 'run.completed');
}

/** Whether the chat's run can be saved as a playbook. */
const replayableRun = (req) => hasReplayableSteps(lastRun(req.params.browserId));

/** The task a chat asks for: its newest message from the person. */
const taskOf = (messages) => [...messages].reverse().find((m) => m?.role === 'user')?.content ?? '';

/** Runs the chat with its live events told to the browser that asked; a run that throws is told as failed. */
async function runLive(browserId, messages, options, live: LiveRun) {
  live.start(typeof taskOf(messages) === 'string' ? taskOf(messages) : '');
  const result = await runChat(browserId, messages, options).catch((err) => {
    live.failed(err);
    throw err;
  });
  live.done(result);
  return result;
}

/** A tool-call listener that keeps each call for the answer and tells the browser it is happening. */
const collecting =
  (toolCalls: any[], live: LiveRun) =>
  ({ name, args }) => (toolCalls.push({ name, args }), live.toolCall({ name, args }));

/** Runs the chat between `run.started` and its end event; a run that throws is announced as failed. */
async function announced(key, browserId, live: LiveRun, run: () => Promise<any>) {
  announceChat(key, browserId, live, 'run.started');
  const result = await run().catch((err) => {
    announceChat(key, browserId, live, 'run.failed', { error: live.safe(err.message) });
    throw err;
  });
  announceEnd(key, browserId, live, result);
  return result;
}

/** Runs the chat, collecting the tool calls it made, and whether the run can be saved as a playbook. */
async function converse(req, messages, task) {
  const toolCalls = [];
  const live = new LiveRun(req.params.browserId, task.secrets);
  const key = getKey(req);
  const result = await announced(key, req.params.browserId, live, () =>
    runLive(req.params.browserId, messages, chatOptions(req, task, toolCalls, live), live),
  );
  return chatAnswer(result, toolCalls, replayableRun(req));
}

/** Keeps the limit outcome in the public answer so clients can offer continuation. */
function chatAnswer(result, toolCalls, replayable: boolean) {
  const answer = {
    text: result.text,
    failed: !!result.failed,
    ...(result.limited && { limited: true }),
    toolCalls,
    replayable,
  };
  return result.data === undefined ? answer : { ...answer, data: result.data };
}

/** The run's options: the key, the task, a listener that keeps and tells each tool call, and the walls it may clear. */
function chatOptions(req, task, toolCalls: any[], live: LiveRun) {
  const key = getKey(req);
  return {
    apiKey: key,
    ...task,
    onToolCall: collecting(toolCalls, live),
    onText: () => {},
    ...walls(key, req.params.browserId),
  };
}
