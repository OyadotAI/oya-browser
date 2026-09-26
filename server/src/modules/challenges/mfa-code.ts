/**
 * Getting a one-time code: reading it out of a message, out of a person's
 * reply, or by polling the mailbox or relay it is sent to.
 */

import { setTimeout as sleep } from 'timers/promises';
import * as inbox from './inbox.ts';
import { chatCompletion } from '../../platform/llm.ts';
import { assertSafeTarget } from '../../platform/net-guard.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import {
  DEFAULT_RELAY_TIMEOUT_MS,
  MAILBOX_CLOCK_SLACK_MS,
  MAX_MESSAGE_CHARS,
  RELAY_POLL_MS,
  RELAY_REQUEST_TIMEOUT_MS,
} from './constants.ts';

/** What the tenant's LLM is told when asked to read a code out of a message. */
const EXTRACT_PROMPT =
  'You extract one-time verification codes from messages. ' +
  'Reply with the code alone and nothing else, no label, no quotes, no explanation. ' +
  'The code is what the reader is meant to type into a website to finish signing in. ' +
  'It is NOT a case or reference number, an account number, a phone number, an amount, a date, or a duration such as "valid for 5 minutes". ' +
  'Codes are usually 4-8 characters and may contain letters. ' +
  'If the message has no such code, reply exactly: NONE';

/** Mailbox readers by factor type; any other type polls its relay URL. */
const INBOX_READERS = { gmail: inbox.gmail, graph: inbox.graph };

/**
 * Pull the code out of a message.
 *
 * A regex is the wrong primary tool here: portals rewrite these templates
 * constantly, codes are not always digits, and a verification email is full of
 * other numbers, a case reference, a phone number, "valid for 5 minutes". The
 * tenant's own LLM reads it instead, with the regex kept as the fallback for
 * self-hosters with no LLM key configured and for when the call fails.
 *
 * Only the one already-matched message is sent, truncated, never a mailbox.
 * It goes to the same provider the tenant's agent runs on, which already sees
 * page content.
 */
export async function extractCode(text, { llm, pattern }: any = {}) {
  const body = String(text || '').slice(0, MAX_MESSAGE_CHARS);
  if (!llm?.openaiKey) return codeByPattern(body, pattern);
  const answer = await codeFromLlm(llm, body);
  return answer === undefined ? codeByPattern(body, pattern) : answer;
}

/** The first match of the caller's pattern, or of a 4-8 digit run. */
function codeByPattern(body, pattern) {
  const match = body.match(pattern || /\b(\d{4,8})\b/);
  return match ? match[1] || match[0] : null;
}

/** The LLM's code, null when it says there is none, or undefined to fall back to the pattern. */
async function codeFromLlm(llm, body) {
  try {
    const answer = await askForCode(llm, body);
    // Trust it only when the answer is shaped like a code. A model that
    // explains itself, or invents one, must not put prose in a login form.
    if (/^[A-Za-z0-9-]{4,10}$/.test(answer) && answer.toUpperCase() !== 'NONE') return answer;
    if (answer.toUpperCase() === 'NONE') return null;
  } catch (e) {
    console.error('[mfa] code extraction fell back to the pattern:', e.message);
  }
  return undefined;
}

/** The LLM's trimmed reply to the extraction prompt. */
async function askForCode(llm, body) {
  const messages = [
    { role: 'system', content: EXTRACT_PROMPT },
    { role: 'user', content: body },
  ];
  const completion = await chatCompletion({ baseUrl: llm.baseUrl, apiKey: llm.openaiKey, model: llm.model, messages });
  return String(completion.choices?.[0]?.message?.content || '').trim();
}

/**
 * Read a one-time code from a mailbox or SMS endpoint.
 *
 * Polled with a bounded window: the code is sent in response to the login
 * attempt, so it does not exist yet when the prompt appears.
 *
 * `since` is what stops the previous run's code being handed back. These codes
 * expire in minutes and the portals offer a Resend button, so an old one is not
 * merely stale, it fails in a way that reads like a broken detector. A relay
 * that cannot say when its message arrived is trusted only for messages it
 * returns after this call started polling.
 */
export async function fetchRelayCode(config, since = 0, llm = null) {
  const deadline = Date.now() + (Number(config.timeoutMs) || DEFAULT_RELAY_TIMEOUT_MS);
  const poll = pollerFor(config, since, llm);
  while (Date.now() < deadline) {
    const code = await attempt(poll);
    if (code) return code;
    await sleep(RELAY_POLL_MS);
  }
  throw new HttpError(Status.GATEWAY_TIMEOUT, 'No one-time code arrived within the window');
}

/** One poll of the factor's source, reading a code out of whatever it returns. */
function pollerFor(config, since, llm) {
  const pattern = config.pattern ? new RegExp(config.pattern) : null;
  const readInbox = Object.hasOwn(INBOX_READERS, config.type) ? INBOX_READERS[config.type] : null;
  const read = (text) => extractCode(text, { llm, pattern });
  return readInbox ? () => pollInbox(readInbox, config, since, read) : () => pollRelay(config, since, read);
}

/** Runs one poll; a failure means try again, except a mailbox that refused its token. */
async function attempt(poll) {
  try {
    return await poll();
  } catch (e) {
    // A revoked refresh token never recovers by polling, and burning the
    // whole window on it hides the one message that would fix it.
    if (e.status === Status.BAD_GATEWAY) throw e;
    return null;
  }
}

/** The code in the newest mailbox message since `since` (give or take the mailbox's clock), if any. */
async function pollInbox(readInbox, config, since, read) {
  const message = await readInbox(config, since ? since - MAILBOX_CLOCK_SLACK_MS : 0);
  return message && (await read(message.text));
}

/** The code the relay URL currently answers with, if it is new enough. */
async function pollRelay(config, since, read) {
  // Re-checked every poll: the name was safe when it was stored, which
  // says nothing about where it resolves now.
  await assertSafeTarget(config.url, { protocols: ['http:', 'https:'], label: 'mfa relay url' });
  const res = await fetch(config.url, {
    headers: config.headers || {},
    redirect: 'error', // a 30x into an internal address would bypass the check above
    signal: AbortSignal.timeout(RELAY_REQUEST_TIMEOUT_MS),
  });
  if (!res.ok) return null;
  return readIfNew(res, since, read);
}

/**
 * An endpoint that timestamps its message is taken at its word; one that does
 * not is only trusted from the second poll onward, by which point anything it
 * returns arrived after this login.
 */
async function readIfNew(res, since, read) {
  const body = await res.text();
  const at = Number(res.headers.get('x-oya-received-at')) || 0;
  return !at || at >= since ? read(body) : null;
}

/**
 * The code in what a person replied with, or null.
 *
 * They answer a parked run in a chat box, so the reply is "445566", or
 * "the code is K7R4QP", or "done" meaning they finished it in the live view
 * themselves. A code always carries a digit, which is what keeps "done",
 * "ok" and "finished" out of a login form.
 */
export function codeInReply(reply) {
  const text = String(reply || '').trim();
  const looksLikeCode = (token) => /^[A-Za-z0-9][A-Za-z0-9-]{3,9}$/.test(token) && /\d/.test(token);
  if (looksLikeCode(text)) return text;
  for (const token of text.split(/[^A-Za-z0-9-]+/)) if (looksLikeCode(token)) return token;
  return null;
}
