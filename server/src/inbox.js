/**
 * Reading a one-time code out of a mailbox.
 *
 * Gmail and Microsoft Graph are both plain HTTPS JSON APIs, so this is fetch and
 * nothing else — no mail client, no new dependency, which is the shape the rest
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

const TOKEN_HOSTS = { google: 'https://oauth2.googleapis.com/token', microsoft: 'https://login.microsoftonline.com' };

/** access tokens live ~1h; a 90s poll window must not re-mint one every 5s. */
const tokens = new Map(); // refreshToken -> { value, expires }

async function accessToken(kind, config) {
  const cached = tokens.get(config.refreshToken);
  if (cached && cached.expires > Date.now() + 60_000) return cached.value;

  const url = kind === 'gmail'
    ? TOKEN_HOSTS.google
    : `${TOKEN_HOSTS.microsoft}/${encodeURIComponent(config.tenant || 'common')}/oauth2/v2.0/token`;
  const body = new URLSearchParams({
    grant_type: 'refresh_token',
    refresh_token: config.refreshToken,
    client_id: config.clientId,
    ...(config.clientSecret ? { client_secret: config.clientSecret } : {}),
    ...(kind === 'graph' ? { scope: 'https://graph.microsoft.com/Mail.Read offline_access' } : {}),
  });
  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body,
    signal: AbortSignal.timeout(15_000),
  });
  if (!res.ok) {
    // The body carries the provider's reason (invalid_grant on a revoked token),
    // which is the one thing that makes this fixable without guessing.
    const detail = await res.text().catch(() => '');
    throw Object.assign(new Error(`Mailbox token refresh failed (${res.status}) ${detail.slice(0, 200)}`), { status: 502 });
  }
  const json = await res.json();
  tokens.set(config.refreshToken, {
    value: json.access_token,
    expires: Date.now() + (Number(json.expires_in) || 3600) * 1000,
  });
  return json.access_token;
}

/** Mail bodies arrive as HTML at least as often as text; the code is in the text. */
export const htmlToText = (html) => String(html)
  .replace(/<(script|style)\b[^>]*>[\s\S]*?<\/\1>/gi, ' ')
  .replace(/<[^>]+>/g, ' ')
  .replace(/&nbsp;/g, ' ')
  .replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'")
  .replace(/\s+/g, ' ')
  .trim();

export const decodeBase64Url = (data) => Buffer.from(String(data).replace(/-/g, '+').replace(/_/g, '/'), 'base64').toString('utf8');

/** Walk a Gmail payload tree for the richest body it has. */
export function gmailBody(payload) {
  if (!payload) return '';
  const parts = [];
  const walk = (node) => {
    if (!node) return;
    if (node.body?.data) parts.push({ type: node.mimeType || '', text: decodeBase64Url(node.body.data) });
    for (const child of node.parts || []) walk(child);
  };
  walk(payload);
  const plain = parts.find((p) => p.type.startsWith('text/plain'));
  if (plain) return plain.text;
  const html = parts.find((p) => p.type.startsWith('text/html'));
  return html ? htmlToText(html.text) : (parts[0]?.text || '');
}

/**
 * The newest message received after `since`, as text.
 * @returns {Promise<{text: string, at: number}|null>}
 */
export async function gmail(config, since = 0) {
  const token = await accessToken('gmail', config);
  const headers = { Authorization: `Bearer ${token}` };
  // Gmail's `after:` takes whole seconds and is inclusive, so it is a coarse
  // prefilter only — internalDate below is what actually enforces the window.
  const query = [config.query || '', `after:${Math.floor(since / 1000)}`].filter(Boolean).join(' ');
  const list = await fetch(
    `https://gmail.googleapis.com/gmail/v1/users/me/messages?maxResults=5&q=${encodeURIComponent(query)}`,
    { headers, signal: AbortSignal.timeout(15_000) },
  );
  if (!list.ok) throw Object.assign(new Error(`Gmail list failed (${list.status})`), { status: 502 });
  const { messages = [] } = await list.json();

  for (const { id } of messages) {
    const res = await fetch(`https://gmail.googleapis.com/gmail/v1/users/me/messages/${encodeURIComponent(id)}?format=full`,
      { headers, signal: AbortSignal.timeout(15_000) });
    if (!res.ok) continue;
    const message = await res.json();
    const at = Number(message.internalDate) || 0;
    if (at < since) continue;
    const subject = message.payload?.headers?.find((h) => h.name?.toLowerCase() === 'subject')?.value || '';
    return { text: `${subject}\n${gmailBody(message.payload)}`, at };
  }
  return null;
}

/** The newest Graph message received after `since`, as text. */
export async function graph(config, since = 0) {
  const token = await accessToken('graph', config);
  // $search cannot be combined with $orderby, so the filter is on time and the
  // caller's query (when given) is applied to the text we already have.
  const filter = `receivedDateTime ge ${new Date(since).toISOString()}`;
  const url = 'https://graph.microsoft.com/v1.0/me/messages'
    + `?$top=5&$orderby=receivedDateTime desc&$select=subject,body,receivedDateTime&$filter=${encodeURIComponent(filter)}`;
  const res = await fetch(url, {
    headers: { Authorization: `Bearer ${token}` },
    signal: AbortSignal.timeout(15_000),
  });
  if (!res.ok) throw Object.assign(new Error(`Graph list failed (${res.status})`), { status: 502 });
  const { value = [] } = await res.json();

  for (const message of value) {
    const at = Date.parse(message.receivedDateTime) || 0;
    if (at < since) continue;
    const body = message.body?.contentType === 'html' ? htmlToText(message.body.content) : (message.body?.content || '');
    const text = `${message.subject || ''}\n${body}`;
    if (config.query && !text.toLowerCase().includes(String(config.query).toLowerCase())) continue;
    return { text, at };
  }
  return null;
}

/** Test hook. */
export function reset() { tokens.clear(); }
