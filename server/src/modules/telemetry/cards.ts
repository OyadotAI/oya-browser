/**
 * The product channel's cards: one block per event worth reading, laid out
 * like A2ABase's #posthog-events (event, who, what happened, when), so both
 * products read the same in Slack. The server has no page, browser or
 * location for an event, so a card carries what it does know instead of
 * blank fields. Every value comes from the typed props or `who.label`; an
 * event with no entry, or whose entry answers null, posts no card.
 */
import { SECONDS_PER_HOUR, SECONDS_PER_MINUTE } from '../../platform/constants.ts';
import { CARD_DIVIDER_CHARS, CARD_MIN_SESSION_SECONDS, PROJECT_ID_CHARS } from './constants.ts';
import type { EventName, EventProps, Who } from './catalog.ts';

/** The rule above and below a card. */
const DIVIDER = '━'.repeat(CARD_DIVIDER_CHARS);

/** How a replay ended, as a card says it. */
const OUTCOMES: Record<EventProps['playbook_replayed']['outcome'], string> = {
  ok: '✅ Ran as saved',
  healed: '🩹 Healed by the agent',
  handed_over: '🙋 Handed to a person',
  failed: '❌ Failed',
};

/** How long something took, as a person reads it: `42s`, `4m 12s`, `2h 5m`. */
export function duration(seconds: number) {
  const s = Math.max(0, Math.round(seconds));
  const [h, m] = [Math.floor(s / SECONDS_PER_HOUR), Math.floor((s % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE)];
  if (h) return `${h}h ${m}m`;
  return m ? `${m}m ${s % SECONDS_PER_MINUTE}s` : `${s}s`;
}

/** The provider line most cards carry. */
const provider = (name: string) => `☁️ Provider: ${name}`;

/** The detail lines of each event's card; null means this one is not worth a card. */
export const CARD_DETAILS: { [K in EventName]?: (props: EventProps[K]) => string[] | null } = {
  account_signed_up: (p) => [`🔐 Method: ${p.method}`],
  agent_signed_up: () => ['🤖 An AI agent signed up for its person'],
  agent_key_claimed: () => ['🤝 Claimed the key an agent signed up for'],
  agent_cloud_refused: (p) => [provider(p.provider), '🚧 Unclaimed agent key asked for a paid browser'],
  api_key_created: (p) => [`📁 Project: ${p.project_id.slice(0, PROJECT_ID_CHARS)}`],
  // Only a key's first browser: that is activation. Every session still ends in a browser_stopped card.
  browser_started: (p) =>
    p.first ? ['🚀 First browser on this key', provider(p.provider), `🧭 Via: ${p.via}`, persona(p.persona)] : null,
  browser_stopped: (p) =>
    p.seconds >= CARD_MIN_SESSION_SECONDS ? [provider(p.provider), `⏱️ Session: ${duration(p.seconds)}`] : null,
  playbook_saved: (p) => [`📋 Steps: ${p.steps}`],
  playbook_replayed: (p) => [`📋 Steps: ${p.steps}`, OUTCOMES[p.outcome]],
  cdp_attached: (p) => [provider(p.provider), '🔌 Playwright or another CDP client attached'],
  // Only the first connect: a laptop waking is not news.
  desktop_connected: (p) => (p.first ? [`💻 Platform: ${p.platform}`, `🏷️ Version: ${p.version}`] : null),
  desktop_updated: (p) => [`💻 Platform: ${p.platform}`, `⬆️ Version: ${p.from} → ${p.to}`],
  // Only a person's download: the app fetching its own update is not news.
  download_served: (p) =>
    p.via === 'web' && p.file_type === 'installer' ? [`💻 Platform: ${p.platform}`, `🏷️ Version: ${p.version}`] : null,
  persona_created: (p) => [`🌐 Proxy: ${p.has_proxy ? 'yes' : 'no'}`],
  server_error: (p) => [`⚠️ Route: ${p.method} ${p.route}`, `🔎 Ref: ${p.ref}`],
};

/** Whether a browser ran as a named persona, as a card says it. */
const persona = (named: boolean) => `🎭 Persona: ${named ? 'named' : 'default'}`;

/** The card for one event about `who`, or null when the event is not worth one. */
export function card<K extends EventName>(name: K, who: Who, props: EventProps[K]): string | null {
  const details = Object.hasOwn(CARD_DETAILS, name) ? CARD_DETAILS[name](props) : null;
  if (!details) return null;
  const head = [DIVIDER, `📌 Event: ${name}`, `👤 User: ${who.label || 'anonymous visitor'}`, ''];
  return [...head, ...details, '', `🕒 Time: ${new Date().toISOString()}`, DIVIDER].join('\n');
}
