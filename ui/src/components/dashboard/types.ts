/**
 * The shapes the dashboard reads from the API (browsers, fleet, personas) and
 * the labels it shows for their enum-like fields.
 */

/** How a browser is doing, as the server judges it from its recent commands and heartbeats. */
export type Health = 'ok' | 'stale' | 'errors' | 'dead';

/** One browser, as GET /browsers and GET /browsers/:id return it. */
export interface BrowserRow {
  /** Stable browser id. */
  id: string;
  /** Display name. */
  name: string;
  /** An Oya client speaks the control socket; a CDP one is driven over the gateway. */
  clientType: 'oya' | 'cdp';
  /** Where it runs (see PROVIDER_LABEL), when known. */
  provider: string | null;
  /** Persona id it runs as. */
  persona: string | null;
  /** Persona name, for display. */
  personaName: string | null;
  /** Current health verdict. */
  health: Health;
  /** When it connected (ISO). */
  connectedAt: string;
  /** When it was last heard from (ISO). */
  lastSeen: string;
  /** The page it is on. */
  currentUrl: string;
  /** Commands run since it connected. */
  commands: number;
  /** Commands that failed. */
  errors: number;
  /** Commands in flight. */
  pending: number;
  /** When the last command ran (ISO). */
  lastCommandAt: string | null;
  /** The last failure's message. */
  lastError: string | null;
  /** Whether someone is watching its live view. */
  streaming: boolean;
}

/** One line of a browser's activity log. */
export interface Activity {
  /** When it happened (ISO). */
  ts: string;
  /** The command name. */
  action: string;
  /** What it did, in a few words. */
  summary: string;
  /** Whether it succeeded. */
  ok: boolean;
  /** How long it took. */
  ms: number;
  /** Why it failed, when it did. */
  error?: string;
}

/** A browser with its recent activity, as GET /browsers/:id returns it. */
export interface BrowserDetail extends BrowserRow {
  /** Recent commands, newest first. */
  activity: Activity[];
}

/** The fleet summary from GET /fleet. */
export interface Fleet {
  /** When it was computed (ISO). */
  at: string;
  /** How long the server has been up. */
  uptimeSeconds: number;
  /** Browser counts, broken down every way the strip filters by. */
  browsers: {
    /** All connected browsers. */
    total: number;
    /** Count per client type. */
    byClient: Record<string, number>;
    /** Count per provider. */
    byProvider: Record<string, number>;
    /** Count per health verdict. */
    byHealth: Record<Health, number>;
    /** Count per persona. */
    byPersona: Record<string, number>;
    /** Commands run across the fleet. */
    commands: number;
    /** Commands failed across the fleet. */
    errors: number;
    /** Commands in flight across the fleet. */
    pending: number;
  };
  /** This hour's metered usage, by meter. */
  usage?: Record<string, number> & UsageHour;
  /** Quotas by name: how much is used and allowed. */
  quotas?: Record<string, Quota>;
  /** The key's limits, as configured. */
  limits?: Record<string, unknown>;
}

/** A persona: one identity (fingerprint, cookies, proxy) browsers run as. */
export interface Persona {
  /** Stable persona id. */
  id: string;
  /** Display name. */
  name: string;
  /** Whether this is the key's default profile. */
  isDefault: boolean;
  /** When it was made (ISO). */
  createdAt: string;
  /** When a browser last ran as it (ISO). */
  lastUsedAt: string | null;
  /** Browsers running as it now. */
  activeBrowsers: number;
  /** How many may run at once; null means no cap. */
  maxConcurrent: number | null;
  /** Its own proxy, if it has one. */
  proxy: PersonaProxy | null;
  /** The pooled exit it is pinned to, if any. */
  exit: PersonaExit | null;
  /** What was asked for when it was made; the fingerprint is what was rolled. */
  prefs: PersonaPrefs | null;
  /** The device it presents, fixed for life. */
  fingerprint: {
    /** navigator.platform. */
    platform: string;
    /** IANA time zone. */
    timezone: string;
    /** BCP 47 locale. */
    locale: string;
    /** Screen size, e.g. "1920x1080". */
    screen: string;
    /** WebGL renderer string. */
    webgl: string;
    /** navigator.hardwareConcurrency. */
    hardwareConcurrency: number;
    /** navigator.deviceMemory. */
    deviceMemory: number;
    /** Seed for canvas noise. */
    canvasSeed: number;
  };
  /** The default MFA factor, without its secret. */
  mfa: PersonaMfa;
  /** Per-site factors and stored logins. Usernames and types only, never secrets. */
  sites?: {
    /** Per-site MFA factors. */
    mfa: SiteMfa[];
    /** Per-site stored logins. */
    credentials: SiteCredential[];
  };
  /** The saved session: how many cookies, for which sites, and when. */
  login?: PersonaLogin;
}

/** Which hour a usage report covers. */
export interface UsageHour {
  /** The hour (ISO), when the server says. */
  hour?: string;
}

/** One quota's standing. */
export interface Quota {
  /** How much is used. */
  current?: number;
  /** How much is allowed. */
  quota?: number;
  /** How much is left. */
  remaining?: number;
}

/** A persona's own proxy, without its credentials. */
export interface PersonaProxy {
  /** Proxy host. */
  host?: string;
  /** Proxy port. */
  port?: number;
  /** Where it exits, when known. */
  geo: string | null;
}

/** A pooled exit a persona is pinned to. */
export interface PersonaExit {
  /** Exit id in the proxy pool. */
  id: string;
  /** Display label. */
  label: string;
  /** Where it exits, when known. */
  geo: string | null;
  /** Whether its last check passed. */
  healthy: boolean;
}

/** What was asked for when a persona was made. */
export interface PersonaPrefs {
  /** Requested platform. */
  platform?: string;
  /** Requested time zone. */
  timezone?: string;
  /** Requested locale. */
  locale?: string;
}

/** A persona's default MFA factor, without its secret. */
export interface PersonaMfa {
  /** Whether one is set. */
  configured: boolean;
  /** Factor type, e.g. totp. */
  type?: string;
  /** The site it is for. */
  domain?: string;
}

/** A per-site MFA factor, without its secret. */
export interface SiteMfa {
  /** The site. */
  domain: string;
  /** Factor type. */
  type: string;
}

/** A per-site stored login, without its password. */
export interface SiteCredential {
  /** The site. */
  domain: string;
  /** The username. */
  username: string;
}

/** A persona's saved session. */
export interface PersonaLogin {
  /** Cookies saved. */
  cookies: number;
  /** Sites they belong to. */
  sites: string[];
  /** When they were last saved (ISO). */
  updatedAt: string | null;
}

/** Human names for provider ids. */
export const PROVIDER_LABEL: Record<string, string> = {
  'oya-cloud': 'Oya Cloud',
  'oya-selfhosted': 'Oya self-hosted',
  'oya-desktop': 'Desktop',
  browseruse: 'Browser Use',
  browserbase: 'Browserbase',
  steel: 'Steel',
  anchor: 'Anchor',
  cdp: 'CDP',
};

/** A provider's human name; an unknown id shows as itself, a missing one as a dash. */
export const providerLabel = (p: string | null | undefined) => (p ? PROVIDER_LABEL[p] || p : '—');

/** Human names for navigator.platform values. */
export const PLATFORM_LABEL: Record<string, string> = {
  Win32: 'Windows',
  MacIntel: 'macOS',
  'Linux x86_64': 'Linux',
};
/** A platform's human name; none means the server picks, shown as "auto". */
export const platformLabel = (p: string | undefined) => (p ? PLATFORM_LABEL[p] || p : 'auto');

/** Words for each health verdict. */
export const HEALTH_LABEL: Record<Health, string> = {
  ok: 'healthy',
  stale: 'stale',
  errors: 'errors',
  dead: 'unresponsive',
};
