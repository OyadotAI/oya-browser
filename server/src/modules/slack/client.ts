/**
 * Talking to Slack: the Web API, the OAuth code exchange, and where the console
 * lives so links in messages open from someone else's machine.
 */
import { API, REQUEST_TIMEOUT_MS, DEFAULT_PORT } from './constants.ts';

/** Slack answers 200 with { ok: false, error }; these mean the install is gone, not that we should retry for a day. */
const DEAD = new Set([
  'invalid_auth',
  'account_inactive',
  'token_revoked',
  'channel_not_found',
  'is_archived',
  'not_in_channel',
]);

/** Whether this deployment has its own Slack app for the OAuth install. */
export const oauthConfigured = () => !!(process.env.SLACK_CLIENT_ID && process.env.SLACK_CLIENT_SECRET);

/**
 * Where the console is reachable from a browser that is not this process. The
 * live link is useless relative — it is read in Slack, on someone else's machine.
 * Derived from the WS URL the same way worker.js derives its provisioning host.
 */
export function consoleUrl() {
  if (process.env.OYA_CONSOLE_URL) return process.env.OYA_CONSOLE_URL.replace(/\/+$/, '');
  const ws = process.env.OYA_PUBLIC_WS_URL;
  if (!ws) return `http://localhost:${process.env.PORT || DEFAULT_PORT}`;
  const url = new URL(ws);
  url.protocol = url.protocol === 'wss:' ? 'https:' : 'http:';
  url.pathname = '/';
  url.search = '';
  return url.href.replace(/\/+$/, '');
}

/** Where OAuth comes back to. Same origin as the console, so nothing here needs CORS. */
export const redirectUri = () => `${consoleUrl()}/api/slack/callback`;

/** One Slack Web API call. Returns the parsed body; `ok` is the thing to check, not the status. */
export async function call(token, method, body?) {
  const response = await fetch(`${API}/${method}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json; charset=utf-8', Authorization: `Bearer ${token}` },
    body: JSON.stringify(body || {}),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  return response.json();
}

/** Whether a Slack error means the install is gone rather than worth retrying. */
export const isDeadInstall = (error) => DEAD.has(error);

/** Trade an OAuth code for a bot token. */
export async function exchangeCode(code) {
  const body = new URLSearchParams({
    code: String(code),
    client_id: process.env.SLACK_CLIENT_ID,
    client_secret: process.env.SLACK_CLIENT_SECRET,
    redirect_uri: redirectUri(),
  });
  const init = { method: 'POST', body, signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS) };
  return fetch(`${API}/oauth.v2.access`, init).then((r) => r.json());
}
