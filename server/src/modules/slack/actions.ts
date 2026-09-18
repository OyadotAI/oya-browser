/**
 * The Resume button. Unauthenticated by necessity — Slack calls it — and trusted
 * only because of the signature over the raw body, which is why index.js parses
 * this one path with express.urlencoded and keeps the bytes.
 */
import { Router } from 'express';
import * as runs from '../playbooks/runs.ts';
import { Status } from '../../platform/http-status.ts';
import { verifySignature } from './signature.ts';
import { REQUEST_TIMEOUT_MS } from './constants.ts';

/** Marks a body that did not parse, since JSON.parse never returns it. */
const UNPARSED = Symbol('unparsed');

/** Parsed JSON, or the fallback when the text is not JSON. */
function parseOr(text, fallback) {
  try {
    return JSON.parse(text);
  } catch {
    return fallback;
  }
}

/** Resume the run a button names; truthy when a waiting run was resumed. */
function resume(action) {
  const value = parseOr(action.value || '{}', {});
  return value.owner && value.runId && runs.respond(value.owner, value.runId, 'done');
}

/** Post a follow-up to a Slack response URL, ignoring failures. */
function postReply(url, text) {
  fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ replace_original: false, text }),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  }).catch(() => {});
}

/** Tell the channel who resumed the run, or that nothing was waiting. Fire and forget. */
function reply(payload, resumed) {
  const who = payload.user?.id ? `<@${payload.user.id}>` : 'someone';
  const text = resumed ? `✅ Resumed by ${who}.` : '⚠️ That run is no longer waiting for anyone.';
  if (payload.response_url) postReply(payload.response_url, text);
}

/** Whether the request carries Slack's signature over its raw body. */
const signed = (req) => verifySignature(req.rawBody?.toString('utf8') ?? '', req.headers);

/**
 * Signed button presses from Slack messages.
 *
 * ponytail: runs live in one replica's memory (runs.js), so this answers only when
 * Slack reaches the replica holding the run — the same ceiling POST /api/runs/:id/respond
 * already has. Moving runs into the control store fixes both.
 */
export const slackActionsRouter = Router();
/** POST /slack/actions — a signed button press from a Slack message. */
slackActionsRouter.post('/', (req, res) => {
  if (!signed(req)) return res.status(Status.UNAUTHORIZED).send('bad signature');
  const payload = parseOr(req.body?.payload || '{}', UNPARSED);
  if (payload === UNPARSED) return res.status(Status.BAD_REQUEST).send('bad payload');
  const action = payload.actions?.[0];
  if (action?.action_id !== 'resume_run') return res.status(Status.OK).send('');
  const resumed = resume(action);
  res.status(Status.OK).send('');
  reply(payload, resumed);
});
