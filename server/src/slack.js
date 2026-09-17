/**
 * Slack notifications: a run that failed, or one parked on a person.
 *
 * Two ways in, one thing stored. A customer either installs the hosted Slack app
 * (OAuth) or pastes a bot token from their own app; both write the same sealed
 * `_slack` row in key-config, so nothing downstream knows which path was used.
 * Sending rides the control plane's existing event -> outbox -> delivery worker,
 * so retries, backoff and replay are the ones that were already there.
 *
 * The message's primary action is a share link — an expiring credential scoped to
 * one browser (control/service.js share()) — so whoever sees the alert can take the
 * browser over and finish the login or CAPTCHA without an Oya account.
 */

import { Router } from 'express';
import { createHmac, randomBytes, timingSafeEqual } from 'node:crypto';
import { authMiddleware } from './auth.js';
import { control, projectId, keyOfProject, fault } from './control/service.js';
import * as keyConfig from './key-config.js';
import { audit } from './audit.js';
import * as runs from './runs.js';

const API = 'https://slack.com/api';
// Deliberately a subset of what the shared getoya.ai Slack app already declares, so
// that one app serves this install too. `channels:join` rather than `chat:write.public`:
// the bot joins the chosen public channel outright instead of posting from outside it.
const SCOPES = 'chat:write,channels:join,channels:read,groups:read';
const STATE_TTL_MS = 5 * 60_000;
/** Slack answers 200 with { ok: false, error }; these mean the install is gone, not that we should retry for a day. */
const DEAD = new Set(['invalid_auth', 'account_inactive', 'token_revoked', 'channel_not_found', 'is_archived', 'not_in_channel']);

export const oauthConfigured = () => !!(process.env.SLACK_CLIENT_ID && process.env.SLACK_CLIENT_SECRET);

/**
 * Where the console is reachable from a browser that is not this process. The
 * live link is useless relative — it is read in Slack, on someone else's machine.
 * Derived from the WS URL the same way worker.js derives its provisioning host.
 */
export function consoleUrl() {
  if (process.env.OYA_CONSOLE_URL) return process.env.OYA_CONSOLE_URL.replace(/\/+$/, '');
  const ws = process.env.OYA_PUBLIC_WS_URL;
  if (!ws) return `http://localhost:${process.env.PORT || 3100}`;
  const url = new URL(ws);
  url.protocol = url.protocol === 'wss:' ? 'https:' : 'http:';
  url.pathname = '/'; url.search = '';
  return url.href.replace(/\/+$/, '');
}

/** One Slack Web API call. Returns the parsed body; `ok` is the thing to check, not the status. */
export async function call(token, method, body) {
  const response = await fetch(`${API}/${method}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json; charset=utf-8', Authorization: `Bearer ${token}` },
    body: JSON.stringify(body || {}),
    signal: AbortSignal.timeout(10000),
  });
  return response.json();
}

export const isDeadInstall = (error) => DEAD.has(error);

// ── Message ──

const TITLES = {
  'run.needs_attention': { emoji: '🔴', text: 'Oya needs you' },
  'run.failed': { emoji: '⚠️', text: 'Run failed' },
  'session.failed': { emoji: '⚠️', text: 'Browser session failed' },
};
const REASONS = { captcha: 'a CAPTCHA', mfa: 'an MFA prompt', agent: 'the agent asked for a person', heal_failed: 'a broken playbook step' };

/** Block Kit for one event. `liveUrl` is absent when the browser is already gone. */
export function blocksFor(event, liveUrl) {
  const d = event.detail || {};
  const title = TITLES[event.type] || { emoji: '⚠️', text: event.type };
  const headline = event.type === 'run.needs_attention' && d.reason
    ? `${title.emoji} ${title.text} — ${REASONS[d.reason] || d.reason}`
    : `${title.emoji} ${title.text}`;
  const context = [event.sessionId && `Browser \`${event.sessionId}\``, d.runId && `\`${d.runId}\``].filter(Boolean).join(' · ');
  const body = d.message || d.error || d.reason || 'No detail was reported.';
  const actions = [];
  if (liveUrl) actions.push({ type: 'button', text: { type: 'plain_text', text: 'Open live browser ↗' }, url: liveUrl, style: 'primary' });
  if (event.type === 'run.needs_attention' && d.runId) {
    actions.push({
      type: 'button', action_id: 'resume_run', text: { type: 'plain_text', text: '✅ Resume run' },
      // Round-tripped by Slack and returned signed, so it cannot be forged without the signing secret.
      value: JSON.stringify({ runId: d.runId, owner: d.owner }),
    });
  }
  const blocks = [
    { type: 'section', text: { type: 'mrkdwn', text: `*${headline}*\n${String(body).slice(0, 2500)}` } },
  ];
  if (context) blocks.push({ type: 'context', elements: [{ type: 'mrkdwn', text: context }] });
  if (actions.length) blocks.push({ type: 'actions', elements: actions });
  return { text: headline, blocks };
}

// ── Interactivity ──

/** Slack's v0 signature over the raw request body, with the five-minute replay window. */
export function verifySignature(rawBody, headers, secret = process.env.SLACK_SIGNING_SECRET) {
  if (!secret) return false;
  const timestamp = headers['x-slack-request-timestamp'], signature = headers['x-slack-signature'];
  if (!timestamp || !signature) return false;
  if (Math.abs(Date.now() / 1000 - Number(timestamp)) > 300) return false;
  const expected = `v0=${createHmac('sha256', secret).update(`v0:${timestamp}:${rawBody}`).digest('hex')}`;
  const a = Buffer.from(expected), b = Buffer.from(String(signature));
  return a.length === b.length && timingSafeEqual(a, b);
}

// ── Routes ──

export const slackRouter = Router();

const mask = (v) => (v ? '••••' + String(v).slice(-4) : '');
const wrap = fn => async (req, res, next) => { try { await fn(req, res); } catch (e) { next(e); } };
const keyOf = req => req.principal.key;

/** Where OAuth comes back to. Same origin as the console, so nothing here needs CORS. */
const redirectUri = () => `${consoleUrl()}/api/slack/callback`;

/**
 * The callback carries no credential of ours — it is Slack redirecting the user's
 * browser — so it is mounted before the auth middleware, and `state` is the proof.
 *
 * The state row alone is not enough. It says which project to install into, so a
 * state issued for the attacker's own project, wrapped in an authorize link and sent
 * to a victim, would file the victim's workspace token under the attacker's project —
 * handing the attacker a bot in someone else's Slack. So the state is also bound to
 * the browser that asked for it: /install drops it in a cookie, and a callback whose
 * cookie does not match the query parameter is refused before anything is exchanged.
 */
slackRouter.get('/callback', wrap(async (req, res) => {
  const { code, state, error } = req.query;
  clearStateCookie(res);
  if (error) return res.redirect(`/dashboard?slack=${encodeURIComponent(String(error))}`);
  if (!matchesStateCookie(req, String(state || ''))) return res.redirect('/dashboard?slack=state_mismatch');
  const key = await redeemState(String(state || ''));
  if (!key || !code) return res.redirect('/dashboard?slack=expired');
  const body = new URLSearchParams({ code: String(code), client_id: process.env.SLACK_CLIENT_ID, client_secret: process.env.SLACK_CLIENT_SECRET, redirect_uri: redirectUri() });
  const result = await fetch(`${API}/oauth.v2.access`, { method: 'POST', body, signal: AbortSignal.timeout(10000) }).then(r => r.json());
  if (!result.ok) return res.redirect(`/dashboard?slack=${encodeURIComponent(result.error || 'install_failed')}`);
  const existing = keyConfig.getSlack(key);
  await keyConfig.saveSlack(key, {
    teamId: result.team?.id || null, teamName: result.team?.name || null,
    botToken: result.access_token, byo: false, installedAt: Date.now(),
    // A reinstall keeps the channel the customer already picked.
    channelId: existing?.channelId || null, channelName: existing?.channelName || null,
  });
  await control().slackSink(key, { channel: existing?.channelId || null, enabled: !!existing?.channelId });
  audit({ action: 'slack.connected', actorKey: key, targetType: 'slack', targetId: result.team?.id, meta: { byo: false }, req });
  res.redirect(existing?.channelId ? '/dashboard?slack=connected' : '/dashboard?slack=pick-channel');
}));

slackRouter.use(authMiddleware);

slackRouter.get('/', wrap(async (req, res) => {
  const install = keyConfig.getSlack(keyOf(req));
  res.json({
    connected: !!install?.botToken,
    oauthAvailable: oauthConfigured(),
    // Shown so it can be pasted into the Slack app's allowed redirect URLs. It moves
    // with OYA_CONSOLE_URL, which on a rotating dev tunnel is every restart.
    redirectUri: oauthConfigured() ? redirectUri() : null,
    teamName: install?.teamName || null,
    channelId: install?.channelId || null,
    channelName: install?.channelName || null,
    byo: !!install?.byo,
    botToken: mask(install?.botToken),
  });
}));

/** Bring your own bot: a pasted token, verified against Slack before it is stored. */
slackRouter.put('/', wrap(async (req, res) => {
  const key = keyOf(req);
  const existing = keyConfig.getSlack(key);
  const raw = String(req.body?.botToken || '').trim();
  // The masked placeholder comes back from the form untouched; it means "keep what is there".
  const botToken = raw && !raw.startsWith('•') ? raw : existing?.botToken;
  if (!botToken) throw fault('missing_token', 'A bot token is required', 400);
  const identity = await call(botToken, 'auth.test');
  if (!identity.ok) throw fault('invalid_token', `Slack rejected that token: ${identity.error}`, 400);
  const channelId = req.body?.channelId === undefined ? existing?.channelId || null : req.body.channelId || null;
  const install = {
    teamId: identity.team_id, teamName: identity.team || existing?.teamName || null,
    botToken, byo: raw && !raw.startsWith('•') ? true : !!existing?.byo, installedAt: existing?.installedAt || Date.now(),
    channelId, channelName: await channelName(botToken, channelId, existing),
  };
  await keyConfig.saveSlack(key, install);
  await control().slackSink(key, { channel: channelId, enabled: !!channelId });
  // A public channel the bot is not in refuses chat.postMessage; joining is free and idempotent.
  if (channelId) await call(botToken, 'conversations.join', { channel: channelId }).catch(() => {});
  audit({ action: 'slack.connected', actorKey: key, targetType: 'slack', targetId: install.teamId, meta: { byo: install.byo }, req });
  res.json({ connected: true, teamName: install.teamName, channelId, channelName: install.channelName, byo: install.byo, botToken: mask(botToken) });
}));

slackRouter.delete('/', wrap(async (req, res) => {
  const key = keyOf(req);
  await keyConfig.clearSlack(key);
  await control().slackSink(key, { enabled: false, channel: null });
  audit({ action: 'slack.disconnected', actorKey: key, targetType: 'slack', req });
  res.json({ connected: false });
}));

slackRouter.get('/channels', wrap(async (req, res) => {
  const install = keyConfig.getSlack(keyOf(req));
  if (!install?.botToken) throw fault('not_connected', 'Connect Slack first', 409);
  const result = await call(install.botToken, 'conversations.list', { types: 'public_channel,private_channel', exclude_archived: true, limit: 200 });
  if (!result.ok) throw fault('slack_error', `Slack refused the channel list: ${result.error}`, 400);
  res.json({ channels: (result.channels || []).map(c => ({ id: c.id, name: c.name, private: !!c.is_private })) });
}));

slackRouter.get('/install', wrap(async (req, res) => {
  if (!oauthConfigured()) throw fault('oauth_unavailable', 'This deployment has no Slack app configured; paste a bot token instead', 501);
  const state = await issueState(keyOf(req));
  setStateCookie(req, res, state);
  const url = new URL('https://slack.com/oauth/v2/authorize');
  url.searchParams.set('client_id', process.env.SLACK_CLIENT_ID);
  url.searchParams.set('scope', SCOPES);
  url.searchParams.set('redirect_uri', redirectUri());
  url.searchParams.set('state', state);
  // The dashboard opens this in a tab, so answer with the URL as well as the redirect.
  if (req.query.json) return res.json({ url: url.href });
  res.redirect(url.href);
}));

/**
 * The Resume button. Unauthenticated by necessity — Slack calls it — and trusted
 * only because of the signature over the raw body, which is why index.js parses
 * this one path with express.urlencoded and keeps the bytes.
 *
 * ponytail: runs live in one replica's memory (runs.js), so this answers only when
 * Slack reaches the replica holding the run — the same ceiling POST /api/runs/:id/respond
 * already has. Moving runs into the control store fixes both.
 */
export const slackActionsRouter = Router();
slackActionsRouter.post('/', (req, res) => {
  if (!verifySignature(req.rawBody?.toString('utf8') ?? '', req.headers)) return res.status(401).send('bad signature');
  let payload;
  try { payload = JSON.parse(req.body?.payload || '{}'); } catch { return res.status(400).send('bad payload'); }
  const action = payload.actions?.[0];
  if (action?.action_id !== 'resume_run') return res.status(200).send('');
  let value;
  try { value = JSON.parse(action.value || '{}'); } catch { value = {}; }
  const resumed = value.owner && value.runId && runs.respond(value.owner, value.runId, 'done');
  res.status(200).send('');
  const who = payload.user?.id ? `<@${payload.user.id}>` : 'someone';
  const text = resumed ? `✅ Resumed by ${who}.` : '⚠️ That run is no longer waiting for anyone.';
  if (payload.response_url) {
    fetch(payload.response_url, {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ replace_original: false, text }), signal: AbortSignal.timeout(10000),
    }).catch(() => {});
  }
});

// ── OAuth state ──

/**
 * The half of the state that lives in the browser that started the install, so a
 * state handed to someone else is worthless. SameSite=Lax still rides Slack's
 * top-level redirect back here; the path confines it to these two routes.
 * ponytail: no cookie parser in this server, and one name is all that is read.
 */
const STATE_COOKIE = 'oya_slack_state';
const readCookie = (req, name) => (req.headers.cookie || '')
  .split(';').map(c => c.trim()).find(c => c.startsWith(`${name}=`))?.slice(name.length + 1);

function setStateCookie(req, res, state) {
  const secure = req.secure || String(req.headers['x-forwarded-proto'] || '').includes('https');
  res.cookie(STATE_COOKIE, state, { httpOnly: true, secure, sameSite: 'lax', maxAge: STATE_TTL_MS, path: '/api/slack' });
}
const clearStateCookie = (res) => res.clearCookie(STATE_COOKIE, { path: '/api/slack' });

/** Constant-time, and false for an absent cookie rather than matching an absent state. */
function matchesStateCookie(req, state) {
  const cookie = readCookie(req, STATE_COOKIE);
  if (!cookie || !state) return false;
  const a = Buffer.from(cookie), b = Buffer.from(state);
  return a.length === b.length && timingSafeEqual(a, b);
}

/**
 * A single-use, five-minute state, kept in the control store rather than in memory
 * so the callback may land on any replica. `expires_at` puts its cleanup on the
 * maintenance sweep that already prunes expired rows.
 */
async function issueState(key) {
  const state = randomBytes(24).toString('base64url');
  await control().store.transact(async tx => {
    // The project id, never the key: the project row already holds the key sealed,
    // so this adds no second copy of a credential at rest.
    tx.put('slack_state', state, { id: state, project: projectId(key), expiresAt: Date.now() + STATE_TTL_MS });
  });
  return state;
}

/** The state's project key, once, or null. Consumed whether or not it had expired. */
async function redeemState(state) {
  if (!state) return null;
  const project = await control().store.transact(async tx => {
    const row = await tx.get('slack_state', state);
    if (!row?.project) return null;
    await tx.delete('slack_state', state);
    return row.expiresAt > Date.now() ? row.project : null;
  });
  return project ? keyOfProject(project) : null;
}

async function channelName(token, channelId, existing) {
  if (!channelId) return null;
  if (existing?.channelId === channelId && existing.channelName) return existing.channelName;
  const info = await call(token, 'conversations.info', { channel: channelId }).catch(() => null);
  return info?.ok ? info.channel?.name || null : null;
}
