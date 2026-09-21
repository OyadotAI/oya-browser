/**
 * /api/slack: connect, configure and disconnect the calling key's Slack install.
 */
import { Router } from 'express';
import { authMiddleware } from '../auth/service.ts';
import { control, fault } from '../control/service.ts';
import * as keyConfig from '../config/service.ts';
import { audit } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { call, oauthConfigured, redirectUri } from './client.ts';
import { clearStateCookie, setStateCookie, issueState } from './oauth-state.ts';
import { completeOAuth, saveByo, mask } from './install.ts';
import { AUTHORIZE_URL, SCOPES, CHANNEL_PAGE } from './constants.ts';

/** /api/slack, connect, configure and disconnect the calling key's Slack install. */
export const slackRouter = Router();

/** The API key the request authenticated with. */
const keyOf = (req) => req.principal.key;

/**
 * The callback carries no credential of ours, it is Slack redirecting the user's
 * browser, so it is mounted before the auth middleware, and `state` is the proof.
 *
 * The state row alone is not enough. It says which project to install into, so a
 * state issued for the attacker's own project, wrapped in an authorize link and sent
 * to a victim, would file the victim's workspace token under the attacker's project,
 * handing the attacker a bot in someone else's Slack. So the state is also bound to
 * the browser that asked for it: /install drops it in a cookie, and a callback whose
 * cookie does not match the query parameter is refused before anything is exchanged.
 */
slackRouter.get('/callback', async (req, res) => {
  clearStateCookie(res);
  res.redirect(await completeOAuth(req));
});

/** Every route below needs an API key; the callback above cannot have one. */
slackRouter.use(authMiddleware);

/** The install's status for the dashboard, with the bot token masked. */
const statusOf = (install) => ({
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

/** GET /slack, the install's status, with the bot token masked. */
slackRouter.get('/', async (req, res) => {
  res.json(statusOf(keyConfig.getSlack(keyOf(req))));
});

/** Bring your own bot: a pasted token, verified against Slack before it is stored. */
slackRouter.put('/', async (req, res) => {
  res.json(await saveByo(req, keyOf(req)));
});

/** DELETE /slack, forget the install and stop sending alerts. */
slackRouter.delete('/', async (req, res) => {
  const key = keyOf(req);
  await keyConfig.clearSlack(key);
  await control().slackSink(key, { enabled: false, channel: null });
  audit({ action: 'slack.disconnected', actorKey: key, targetType: 'slack', req });
  res.json({ connected: false });
});

/** One page of the channels a bot token can see, as Slack answers it. */
function fetchChannels(token) {
  const query = { types: 'public_channel,private_channel', exclude_archived: true, limit: CHANNEL_PAGE };
  return call(token, 'conversations.list', query);
}

/** The channels an install's bot can see. */
async function listChannels(install) {
  if (!install?.botToken) throw fault('not_connected', 'Connect Slack first', Status.CONFLICT);
  const result = await fetchChannels(install.botToken);
  if (!result.ok) throw fault('slack_error', `Slack refused the channel list: ${result.error}`, Status.BAD_REQUEST);
  return (result.channels || []).map((c) => ({ id: c.id, name: c.name, private: !!c.is_private }));
}

/** GET /slack/channels, the channels the bot can see, to pick where alerts go. */
slackRouter.get('/channels', async (req, res) => {
  res.json({ channels: await listChannels(keyConfig.getSlack(keyOf(req))) });
});

/** The Slack authorize URL for one state. */
function authorizeUrl(state) {
  const url = new URL(AUTHORIZE_URL);
  url.searchParams.set('client_id', process.env.SLACK_CLIENT_ID);
  url.searchParams.set('scope', SCOPES);
  url.searchParams.set('redirect_uri', redirectUri());
  url.searchParams.set('state', state);
  return url.href;
}

/** Refuse the OAuth install on a deployment without its own Slack app. */
function requireOAuth() {
  if (oauthConfigured()) return;
  const message = 'This deployment has no Slack app configured; paste a bot token instead';
  throw fault('oauth_unavailable', message, Status.NOT_IMPLEMENTED);
}

/** GET /slack/install, start the OAuth install: redirects to Slack, or returns the URL with ?json. */
slackRouter.get('/install', async (req, res) => {
  requireOAuth();
  const state = await issueState(keyOf(req));
  setStateCookie(req, res, state);
  const url = authorizeUrl(state);
  // The dashboard opens this in a tab, so answer with the URL as well as the redirect.
  if (req.query.json) return res.json({ url });
  res.redirect(url);
});
