/**
 * Building and storing a key's Slack install, from either way in: the OAuth
 * callback or a pasted bot token.
 */
import { control, fault } from '../control/service.ts';
import * as keyConfig from '../config/service.ts';
import { audit } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { call, exchangeCode } from './client.ts';
import { matchesStateCookie, redeemState } from './oauth-state.ts';
import { MASK_TAIL } from './constants.ts';

/** A bot token as the dashboard sees it. */
export const mask = (v) => (v ? '••••' + String(v).slice(-MASK_TAIL) : '');

/** Record a connected install in the audit trail. */
const auditConnected = (req, key, targetId, byo) =>
  audit({ action: 'slack.connected', actorKey: key, targetType: 'slack', targetId, meta: { byo }, req });

/** The install an OAuth exchange produced. A reinstall keeps the channel the customer already picked. */
const oauthInstall = (result, existing) => ({
  teamId: result.team?.id || null,
  teamName: result.team?.name || null,
  botToken: result.access_token,
  byo: false,
  installedAt: Date.now(),
  channelId: existing?.channelId || null,
  channelName: existing?.channelName || null,
});

/** Store an OAuth install and point alerts at its channel; returns where the dashboard goes next. */
async function saveOAuthInstall(req, key, result) {
  const existing = keyConfig.getSlack(key);
  await keyConfig.saveSlack(key, oauthInstall(result, existing));
  await control().slackSink(key, { channel: existing?.channelId || null, enabled: !!existing?.channelId });
  auditConnected(req, key, result.team?.id, false);
  return existing?.channelId ? '/dashboard?slack=connected' : '/dashboard?slack=pick-channel';
}

/** Finish the OAuth install from Slack's redirect; returns the dashboard URL to land on. */
export async function completeOAuth(req) {
  const { code, state, error } = req.query;
  if (error) return `/dashboard?slack=${encodeURIComponent(String(error))}`;
  if (!matchesStateCookie(req, String(state || ''))) return '/dashboard?slack=state_mismatch';
  const key = await redeemState(String(state || ''));
  if (!key || !code) return '/dashboard?slack=expired';
  const result = await exchangeCode(code);
  if (!result.ok) return `/dashboard?slack=${encodeURIComponent(result.error || 'install_failed')}`;
  return saveOAuthInstall(req, key, result);
}

/** Check a bot token with Slack, refusing a missing or rejected one. */
async function verifyToken(botToken) {
  if (!botToken) throw fault('missing_token', 'A bot token is required', Status.BAD_REQUEST);
  const identity = await call(botToken, 'auth.test');
  if (!identity.ok) throw fault('invalid_token', `Slack rejected that token: ${identity.error}`, Status.BAD_REQUEST);
  return identity;
}

/** The channel alerts go to: the one sent, or the saved one when none was sent. */
const chosenChannel = (req, existing) =>
  req.body?.channelId === undefined ? existing?.channelId || null : req.body.channelId || null;

/** The channel's name, reused from the saved install when the channel is unchanged; null if Slack will not say. */
async function channelName(token, channelId, existing) {
  if (!channelId) return null;
  if (existing?.channelId === channelId && existing.channelName) return existing.channelName;
  const info = await call(token, 'conversations.info', { channel: channelId }).catch(() => null);
  return info?.ok ? info.channel?.name || null : null;
}

/** The workspace a verified token belongs to. */
const teamOf = (identity, existing) => ({
  teamId: identity.team_id,
  teamName: identity.team || existing?.teamName || null,
});

/** The token to use: a freshly pasted one, else the saved one. */
function tokenFrom(req, existing) {
  const raw = String(req.body?.botToken || '').trim();
  // The masked placeholder comes back from the form untouched; it means "keep what is there".
  const pasted = raw && !raw.startsWith('•');
  return { botToken: pasted ? raw : existing?.botToken, pasted };
}

/** The install a pasted (or kept) bot token makes, verified against Slack. */
async function byoInstall(req, existing) {
  const { botToken, pasted } = tokenFrom(req, existing);
  const identity = await verifyToken(botToken);
  const channelId = chosenChannel(req, existing);
  const since = { byo: pasted ? true : !!existing?.byo, installedAt: existing?.installedAt || Date.now() };
  const name = await channelName(botToken, channelId, existing);
  return { ...teamOf(identity, existing), botToken, ...since, channelId, channelName: name };
}

/** Point alerts at the install's channel, joining it so posts are accepted. */
async function activate(key, install) {
  await control().slackSink(key, { channel: install.channelId, enabled: !!install.channelId });
  // A public channel the bot is not in refuses chat.postMessage; joining is free and idempotent.
  if (install.channelId)
    await call(install.botToken, 'conversations.join', { channel: install.channelId }).catch(() => {});
}

/** What the dashboard is told about a freshly saved install. */
const savedView = (install) => ({
  connected: true,
  teamName: install.teamName,
  channelId: install.channelId,
  channelName: install.channelName,
  byo: install.byo,
  botToken: mask(install.botToken),
});

/** Save a bring-your-own-bot install from a PUT /slack body; returns what to answer. */
export async function saveByo(req, key) {
  const install = await byoInstall(req, keyConfig.getSlack(key));
  await keyConfig.saveSlack(key, install);
  await activate(key, install);
  auditConnected(req, key, install.teamId, install.byo);
  return savedView(install);
}
