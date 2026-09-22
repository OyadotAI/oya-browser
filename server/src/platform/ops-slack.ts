/**
 * One-line ops messages to Slack incoming webhooks: best effort, never awaited,
 * never retried, dropped on any failure, and a no-op without a webhook. Not
 * evidence and not alerting; nothing here is recorded. Each channel is one
 * env var holding its webhook URL, so a self-host that sets none sends nothing.
 */
import { MESSAGE_MAX_CHARS, OUTBOUND_TIMEOUT_MS } from './constants.ts';

/** The channels a message can go to. */
export type Channel = 'signups' | 'events';

/** The env var that holds each channel's webhook. */
const WEBHOOKS: Record<Channel, string> = {
  signups: 'SLACK_OPS_WEBHOOK_SIGNUPS',
  events: 'SLACK_OPS_WEBHOOK_EVENTS',
};

/** The channel's webhook URL, read at call time; undefined when the operator set none. */
const webhookFor = (channel: Channel) => process.env[WEBHOOKS[channel]] || undefined;

/** Whether the channel is configured. */
export const enabled = (channel: Channel) => webhookFor(channel) !== undefined;

/**
 * The text as Slack may show it. Webhook text is Slack markup, where `<!channel>`
 * pings everyone and `<url|label>` is a link, and parts of a line come from
 * names people typed, so the three characters that carry meaning are escaped
 * the way Slack documents, and a line is never longer than one of ours.
 */
export const plain = (text: string) =>
  text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').slice(0, MESSAGE_MAX_CHARS);

/** Posts one line to the channel and forgets about it. */
export function post(channel: Channel, text: string) {
  const url = webhookFor(channel);
  if (!url) return;
  void fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ text: plain(text) }),
    signal: AbortSignal.timeout(OUTBOUND_TIMEOUT_MS),
  }).catch(() => {});
}
