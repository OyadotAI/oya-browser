/**
 * Routes that drive a browser: one action, or a natural-language chat with
 * the browser tools.
 */
import { sendCommand } from '../socket.ts';
import { runChat, lastRun, hasReplayableSteps, requireLlm } from '../../agent/chat.ts';
import * as usage from '../../../platform/usage.ts';
import { Status } from '../../../platform/http-status.ts';
import { HttpError, answerFor, sendError } from '../../../platform/errors.ts';
import * as flow from '../../playbooks/flow-recorder.ts';
import { getKey, longJson, validData } from '../../../app/http.ts';
import { noTimeouts, refuseAction } from './helpers.ts';
import { MAX_SCHEMA_CHARS } from '../constants.ts';
import { challengesFor, quietCheckpointFor } from '../../playbooks/checkpoint.ts';

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
  await longJson(res, () => converse(req, messages, { data, secrets, schema, signal }));
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

/** Runs the chat, collecting the tool calls it made, and whether the run can be saved as a playbook. */
async function converse(req, messages, task) {
  const toolCalls = [];
  const onToolCall = ({ name, args }) => toolCalls.push({ name, args });
  const key = getKey(req);
  const options = { apiKey: key, ...task, onToolCall, onText: () => {}, ...walls(key, req.params.browserId) };
  const result = await runChat(req.params.browserId, messages, options);
  const replayable = hasReplayableSteps(lastRun(req.params.browserId));
  const answer = { text: result.text, failed: !!result.failed, toolCalls, replayable };
  return result.data === undefined ? answer : { ...answer, data: result.data };
}
