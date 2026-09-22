/**
 * Every product event by name, with the properties each carries and the one
 * Slack line some of them also post. Names are snake_case noun_verb. A property
 * is a small typed value; never a key, a URL, a name a person chose or page
 * content, so the catalog itself is the privacy rule.
 */
import { PROJECT_ID_CHARS } from './constants.ts';

/** Who an event is about, as an ops line names them: an email when the key has an owner, else a short key fingerprint. */
export type Who = {
  /** The PostHog distinct id: the user id, or the key fingerprint. */
  id: string;
  /** The person's email when known. */
  email?: string;
  /** What an ops line prints. */
  label: string;
};

/** The properties of each event. */
export type EventProps = {
  /** A person made an account. */
  account_signed_up: Record<string, never>;
  /** A signed-in person made an API key. */
  api_key_created: {
    /** The project the key belongs to. */
    project_id: string;
  };
  /** A browser started for the key. */
  browser_started: {
    /** Which provider ran it. */
    provider: string;
    /** Whether it ran as a named persona. */
    persona: boolean;
    /** Which client asked for it. */
    via: 'rest' | 'mcp' | 'console';
  };
  /** A browser the key held is gone. */
  browser_stopped: {
    /** Which provider ran it. */
    provider: string;
    /** How long it was connected. */
    seconds: number;
  };
  /** A playbook was saved. */
  playbook_saved: {
    /** How many steps it holds. */
    steps: number;
  };
  /** A playbook was replayed. */
  playbook_replayed: {
    /** How the run ended. */
    outcome: 'ok' | 'healed' | 'handed_over' | 'failed';
    /** How many steps it holds. */
    steps: number;
    /** Whether a step had to be repaired. */
    healed: boolean;
  };
  /** An MCP tool ran. */
  mcp_tool_called: {
    /** The tool's name. */
    tool: string;
  };
  /** A CDP client attached through the gateway. */
  cdp_attached: {
    /** Which provider the browser runs on. */
    provider: string;
  };
  /** The desktop app connected. */
  desktop_connected: {
    /** The desktop's platform, such as MacIntel. */
    platform: string;
    /** Whether this key had never connected a desktop before. */
    first: boolean;
  };
  /** A persona was created. */
  persona_created: {
    /** Whether it was given a proxy. */
    has_proxy: boolean;
  };
  /** The server answered a 500 under a reference. */
  server_error: {
    /** The reference in the body and the log line. */
    ref: string;
    /** The HTTP method. */
    method: string;
    /** The route template, or `middleware` when none matched. */
    route: string;
  };
};

/** An event name. */
export type EventName = keyof EventProps;

/** The channel an event's Slack line goes to. */
export const CHANNEL: Partial<Record<EventName, 'signups' | 'events'>> = {
  account_signed_up: 'signups',
  api_key_created: 'signups',
  desktop_connected: 'signups',
  playbook_saved: 'events',
  cdp_attached: 'events',
  server_error: 'events',
};

/** `n step` or `n steps`. */
const steps = (n: number) => `${n} step${n === 1 ? '' : 's'}`;

/** The Slack line for each event that has one. Every value printed comes from the typed props or `who.label`. */
export const SLACK_LINES: { [K in EventName]?: (who: Who, props: EventProps[K]) => string | null } = {
  account_signed_up: (who) => `🎉 New signup: ${who.label}`,
  api_key_created: (who, p) => `🔑 API key created: ${who.label} (project ${p.project_id.slice(0, PROJECT_ID_CHARS)})`,
  // Only the first connect: a laptop waking is not news.
  desktop_connected: (who, p) => (p.first ? `🖥️ Desktop connected: ${who.label} (${p.platform})` : null),
  playbook_saved: (who, p) => `💾 Playbook saved: ${who.label} (${steps(p.steps)})`,
  cdp_attached: (who, p) => `🔌 CDP attached: ${who.label} (${p.provider})`,
  server_error: (who, p) =>
    `⚠️ Server error: ${p.method} ${p.route} (ref ${p.ref})${who.label ? ` for ${who.label}` : ''}`,
};
