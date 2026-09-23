/**
 * The one place a product event is emitted from. Each `track` function is a
 * seam another module calls beside its existing audit or usage call; it
 * returns at once, resolves who the event is about in the background, sends
 * the event to PostHog and, for the few events that have one, a line to the
 * ops Slack channel. Nothing here is awaited by a request and nothing here is
 * evidence.
 */
import * as analytics from '../../platform/analytics.ts';
import * as slack from '../../platform/ops-slack.ts';
import { CHANNEL, SLACK_LINES, type EventName, type EventProps, type Who } from './catalog.ts';
import { nobody, person, whoHolds } from './who.ts';

/** Forgets who was identified, so one test cannot leak into the next. */
export const forgetIdentifiedForTests = () => identified.clear();
import { CLIENTS, CLIENT_HEADER } from './constants.ts';

/** People already identified this process: an identify is PostHog's costly merge, so once is enough. */
const identified = new Set<string>();

/** Attaches the email to the person the first time they are seen. */
function identifyOnce(who: Who) {
  if (!who.email || identified.has(who.id)) return;
  identified.add(who.id);
  analytics.identify(who.id, { email: who.email });
}

/** Sends one event about `who` to PostHog, and its Slack line when it has one. */
function send<K extends EventName>(name: K, who: Who, props: EventProps[K]) {
  const profile = who.email !== undefined;
  analytics.capture(who.id, name, { ...props, $process_person_profile: profile });
  identifyOnce(who);
  const line = Object.hasOwn(SLACK_LINES, name) ? SLACK_LINES[name](who, props) : null;
  if (line) slack.post(CHANNEL[name], line);
}

/** Someone who fetched a file: a stable fingerprint, never a person profile, so downloads count without naming anyone. */
const visitorWho = (visitor: string): Who => ({ id: visitor, label: '' });

/** Emits an event about whoever holds `key`, now or once the owner is known; never awaited. */
function emit<K extends EventName>(name: K, key: string | null, props: EventProps[K]) {
  if (!analytics.enabled() && !slackWanted(name)) return;
  const who = key ? whoHolds(key) : nobody;
  if (who instanceof Promise) void who.then((w) => send(name, w, props)).catch(() => {});
  else send(name, who, props);
}

/** Whether the event's Slack channel is configured, so nothing is resolved for an event nobody will see. */
const slackWanted = (name: EventName) => Object.hasOwn(CHANNEL, name) && slack.enabled(CHANNEL[name]);

/** Which client a start came from, from the header it named itself in; anything else is plain REST. */
export function clientOf(headers: Record<string, unknown> = {}): EventProps['browser_started']['via'] {
  const named = String(headers[CLIENT_HEADER] || '');
  return CLIENTS.has(named) ? (named as 'mcp' | 'console') : 'rest';
}

/** A signed-in person, as a route already has them. */
type Person = {
  /** The user id. */
  id: string;
  /** The email, when the profile carries one. */
  email?: string | null;
};

/** The seams. Each takes what the caller already has in hand and returns at once. */
export const track = {
  /** A person made an account. */
  accountSignedUp: (user: Person) => send('account_signed_up', person(user.id, user.email), {}),
  /** A signed-in person made an API key. */
  apiKeyCreated: (user: Person, projectId: string) =>
    send('api_key_created', person(user.id, user.email), { project_id: projectId }),
  /** A browser started for the key. */
  browserStarted: (key: string, props: EventProps['browser_started']) => emit('browser_started', key, props),
  /** A browser the key held is gone. */
  browserStopped: (key: string, props: EventProps['browser_stopped']) => emit('browser_stopped', key, props),
  /** A playbook was saved. */
  playbookSaved: (key: string, props: EventProps['playbook_saved']) => emit('playbook_saved', key, props),
  /** A playbook was replayed. */
  playbookReplayed: (key: string, props: EventProps['playbook_replayed']) => emit('playbook_replayed', key, props),
  /** An MCP tool ran. */
  mcpToolCalled: (key: string, props: EventProps['mcp_tool_called']) => emit('mcp_tool_called', key, props),
  /** Playwright or another CDP client attached through the gateway. */
  cdpAttached: (key: string, props: EventProps['cdp_attached']) => emit('cdp_attached', key, props),
  /** The desktop app connected. */
  desktopConnected: (key: string, props: EventProps['desktop_connected']) => emit('desktop_connected', key, props),
  /** A key's desktop came back on another version. */
  desktopUpdated: (key: string, props: EventProps['desktop_updated']) => emit('desktop_updated', key, props),
  /** A file was served from /downloads, to `visitor` (a fingerprint of who asked, not a person). */
  downloadServed: (visitor: string, props: EventProps['download_served']) =>
    send('download_served', visitorWho(visitor), props),
  /** An installed app checked for an update. */
  updateChecked: (visitor: string, props: EventProps['update_checked']) =>
    send('update_checked', visitorWho(visitor), props),
  /** A persona was created. */
  personaCreated: (key: string, props: EventProps['persona_created']) => emit('persona_created', key, props),
  /** The server answered a 500 under a reference. */
  serverError: (key: string | null, props: EventProps['server_error']) => emit('server_error', key, props),
};
