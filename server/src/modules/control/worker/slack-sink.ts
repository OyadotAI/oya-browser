/**
 * Slack as a delivery sink: one event as a channel message, with a live link
 * when the event is about a browser.
 */
import { keyOfProject } from '../service.ts';
import * as slack from '../../slack/service.ts';
import * as keyConfig from '../../config/service.ts';
import { SLACK_LINK_TTL_S } from './constants.ts';

/** Not sent, but worth retrying. */
const retry = () => ({ ok: false, dead: false });

/**
 * One event as a Slack message. The bot token lives in the project key's sealed
 * settings rather than on the hook row, so an OAuth install and a pasted token
 * arrive here identically. The live link is minted per message and expires in an
 * hour: a share credential scoped to that one browser, which is what makes the
 * alert actionable for someone with no Oya account.
 */
export async function postSlack(service, hook, event) {
  const key = await keyOfProject(hook.project, service);
  if (!key) return retry();
  const install = keyConfig.getSlack(key);
  const channel = hook.channel || install?.channelId;
  // Settings sealed under a rotated secret read as absent, which is recoverable,
  // retry rather than disabling a sink the customer never touched. Disconnecting
  // disables the hook itself, and those deliveries are cancelled before they reach here.
  if (!install?.botToken || !channel) return retry();
  const liveUrl = await liveLink(service, key, event);
  const result = await slack.call(install.botToken, 'chat.postMessage', message(channel, event, liveUrl));
  return { ok: !!result?.ok, dead: !result?.ok && slack.isDeadInstall(result?.error) };
}

/** The chat.postMessage arguments. */
const message = (channel, event, liveUrl) => ({ channel, unfurl_links: false, ...slack.blocksFor(event, liveUrl) });

/** A console link that lets the reader watch and take over the event's browser, or null. */
async function liveLink(service, key, event) {
  if (!event.sessionId) return null;
  // Fails for a browser that has already ended, then the message goes out without the button.
  const share = await service
    .share(key, { id: event.sessionId, control: true, expiresIn: SLACK_LINK_TTL_S })
    .catch(() => null);
  if (!share) return null;
  return `${slack.consoleUrl()}/live/${encodeURIComponent(event.sessionId)}#t=${encodeURIComponent(share.token)}`;
}
