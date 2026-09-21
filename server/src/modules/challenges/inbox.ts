/**
 * Reading a one-time code out of a mailbox.
 *
 * Gmail and Microsoft Graph are both plain HTTPS JSON APIs, so this is fetch and
 * nothing else, no mail client, no new dependency, which is the shape the rest
 * of this server keeps.
 *
 * Every read is bounded by `since`: the code is sent in answer to the login that
 * just happened, so a message older than that submission is the *previous*
 * run's code. Portals expire these in minutes and offer a Resend button, so
 * handing back a stale one fails in a way that reads like a broken detector.
 *
 * Refresh tokens are credential material. They are sealed by the caller
 * (mfa.js), never logged, and never returned by the API.
 */

import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { accessToken, clearTokens } from './mailbox-token.ts';
import { MAILBOX_REQUEST_TIMEOUT_MS, MS_PER_SECOND } from './constants.ts';

/** Gmail's message collection for the signed-in user. */
const GMAIL_MESSAGES = 'https://gmail.googleapis.com/gmail/v1/users/me/messages';
/** Graph's message collection for the signed-in user. */
const GRAPH_MESSAGES = 'https://graph.microsoft.com/v1.0/me/messages';

/** HTML → text rewrites, in order: drop scripts and tags, decode entities, squeeze whitespace. */
const HTML_REWRITES: [RegExp, string][] = [
  [/<(script|style)\b[^>]*>[\s\S]*?<\/\1>/gi, ' '],
  [/<[^>]+>/g, ' '],
  [/&nbsp;/g, ' '],
  [/&amp;/g, '&'],
  [/&lt;/g, '<'],
  [/&gt;/g, '>'],
  [/&quot;/g, '"'],
  [/&#39;/g, "'"],
  [/\s+/g, ' '],
];

/** Mail bodies arrive as HTML at least as often as text; the code is in the text. */
export const htmlToText = (html) =>
  HTML_REWRITES.reduce((text, [pattern, to]) => text.replace(pattern, to), String(html)).trim();

/** Decodes Gmail's base64url body data to UTF-8 text. */
export const decodeBase64Url = (data) =>
  Buffer.from(String(data).replace(/-/g, '+').replace(/_/g, '/'), 'base64').toString('utf8');

/** Walk a Gmail payload tree for the richest body it has. */
export function gmailBody(payload) {
  if (!payload) return '';
  const parts = bodyParts(payload);
  const plain = parts.find((p) => p.type.startsWith('text/plain'));
  if (plain) return plain.text;
  const html = parts.find((p) => p.type.startsWith('text/html'));
  return html ? htmlToText(html.text) : parts[0]?.text || '';
}

/** Every part with a body, depth first, as `{ type, text }`. */
function bodyParts(node, parts = []) {
  if (!node) return parts;
  if (node.body?.data) parts.push({ type: node.mimeType || '', text: decodeBase64Url(node.body.data) });
  for (const child of node.parts || []) bodyParts(child, parts);
  return parts;
}

/**
 * The newest message received after `since`, as text.
 * @returns {Promise<{text: string, at: number}|null>}
 */
export async function gmail(config, since = 0) {
  const headers = { Authorization: `Bearer ${await accessToken('gmail', config)}` };
  for (const { id } of await gmailMessageIds(config, since, headers)) {
    const message = await gmailMessage(id, headers);
    const at = Number(message?.internalDate) || 0;
    if (!message || at < since) continue;
    return { text: `${gmailSubject(message)}\n${gmailBody(message.payload)}`, at };
  }
  return null;
}

/** Ids of the newest messages matching the configured query. */
async function gmailMessageIds(config, since, headers) {
  // Gmail's `after:` takes whole seconds and is inclusive, so it is a coarse
  // prefilter only, internalDate is what actually enforces the window.
  const query = [config.query || '', `after:${Math.floor(since / MS_PER_SECOND)}`].filter(Boolean).join(' ');
  const url = `${GMAIL_MESSAGES}?maxResults=5&q=${encodeURIComponent(query)}`;
  const list = await fetch(url, { headers, signal: AbortSignal.timeout(MAILBOX_REQUEST_TIMEOUT_MS) });
  if (!list.ok) throw new HttpError(Status.BAD_GATEWAY, `Gmail list failed (${list.status})`);
  const { messages = [] } = await list.json();
  return messages;
}

/** One full Gmail message, or null when it cannot be read. */
async function gmailMessage(id, headers) {
  const res = await fetch(`${GMAIL_MESSAGES}/${encodeURIComponent(id)}?format=full`, {
    headers,
    signal: AbortSignal.timeout(MAILBOX_REQUEST_TIMEOUT_MS),
  });
  return res.ok ? res.json() : null;
}

/** A Gmail message's subject header, or ''. */
const gmailSubject = (message) =>
  message.payload?.headers?.find((h) => h.name?.toLowerCase() === 'subject')?.value || '';

/** The newest Graph message received after `since`, as text. */
export async function graph(config, since = 0) {
  const token = await accessToken('graph', config);
  for (const message of await graphMessages(token, since)) {
    const found = graphMatch(message, since, config.query);
    if (found) return found;
  }
  return null;
}

/** The newest Graph messages received since `since`. */
async function graphMessages(token, since) {
  const res = await fetch(graphUrl(since), {
    headers: { Authorization: `Bearer ${token}` },
    signal: AbortSignal.timeout(MAILBOX_REQUEST_TIMEOUT_MS),
  });
  if (!res.ok) throw new HttpError(Status.BAD_GATEWAY, `Graph list failed (${res.status})`);
  const { value = [] } = await res.json();
  return value;
}

/** The newest-first message query for everything received since `since`. */
function graphUrl(since) {
  // $search cannot be combined with $orderby, so the filter is on time and the
  // caller's query (when given) is applied to the text we already have.
  const filter = `receivedDateTime ge ${new Date(since).toISOString()}`;
  return (
    GRAPH_MESSAGES +
    `?$top=5&$orderby=receivedDateTime desc&$select=subject,body,receivedDateTime&$filter=${encodeURIComponent(filter)}`
  );
}

/** A Graph message as `{ text, at }` when it is recent enough and matches the query, else null. */
function graphMatch(message, since, query) {
  const at = Date.parse(message.receivedDateTime) || 0;
  if (at < since) return null;
  const body = message.body?.contentType === 'html' ? htmlToText(message.body.content) : message.body?.content || '';
  const text = `${message.subject || ''}\n${body}`;
  if (query && !text.toLowerCase().includes(String(query).toLowerCase())) return null;
  return { text, at };
}

/** Test hook. */
export function reset() {
  clearTokens();
}
