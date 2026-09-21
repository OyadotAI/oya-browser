/**
 * Types for identities: a persona's device and fingerprint, the proxies it
 * exits through, and the second factors and site logins stored for it.
 */

/** The device a persona presents. Fixed for its life. */
export interface Fingerprint {
  /** `navigator.platform`. */
  platform: string;
  /** IANA timezone. */
  timezone: string;
  /** BCP 47 locale. */
  locale: string;
  /** Screen size, as `WIDTHxHEIGHT`. */
  screen: string;
  /** The GPU the WebGL renderer reports. */
  webgl: string;
  /** Logical CPU cores reported. */
  hardwareConcurrency: number;
  /** Device memory reported, in GB. */
  deviceMemory: number;
  /** Seed for the persona's stable canvas noise. */
  canvasSeed: number;
}

/** Device choices made at creation. Fixed for the persona's life. */
export interface PersonaPrefs {
  /** Which operating system to present. */
  platform?: 'Win32' | 'MacIntel' | 'Linux x86_64';
  /** IANA timezone; must be one the platform can coherently claim. */
  timezone?: string;
  /** BCP 47 locale; must be one the platform can coherently claim. */
  locale?: string;
}

/** A proxy exit. Credentials go in on create and never come back out. */
export interface ProxyInfo {
  /** The proxy's id. */
  id: string;
  /** Its display name. */
  label: string;
  /** Residential IPs look like homes; datacenter ones are cheaper and easier to flag. */
  kind: 'residential' | 'datacenter';
  /** Two-letter country, optionally a region: "US", "US-CA". */
  geo: string | null;
  /** Provided by the host rather than this key. Cannot be removed. */
  shared: boolean;
  /** Whether its last check passed. */
  healthy: boolean;
  /** Healthy and not cooling down after a failure. */
  available: boolean;
  /** Where traffic actually leaves, as of the last check. */
  exitIp: string | null;
  /** When it was last checked. */
  lastCheckedAt: string | null;
  /** Personas on it now, out of `maxPersonas`. */
  assigned: number;
  /** How many personas may share it. */
  maxPersonas: number;
  /** How long until a failed proxy is tried again. */
  cooldownMsRemaining: number;
}

/** A proxy to add. */
export interface ProxyCreate {
  /** http(s)://user:pass@host:port from your vendor. Chromium cannot use SOCKS5 with a password. */
  url: string;
  /** A display name. */
  label?: string;
  /** Two-letter country, optionally a region, used to match personas' geo hints. */
  geo?: string;
  /** Residential or datacenter. */
  kind?: 'residential' | 'datacenter';
  /** Personas that may share it. Keep 1 for a sticky-session URL so each keeps its own IP. */
  maxPersonas?: number;
}

/** A persona: one device, its cookie jar and its proxy. */
export interface PersonaInfo {
  /** The persona's id. */
  id: string;
  /** Its display name. */
  name: string;
  /** Whether it is this key's own default persona. */
  isDefault: boolean;
  /** Browsers running as it now. */
  activeBrowsers: number;
  /** How many may run as it at once; null for no cap. */
  maxConcurrent: number | null;
  /** Where its proxy should be, before one is assigned. */
  proxy: {
    /** Two-letter country, optionally a region. */
    geo: string | null;
  } | null;
  /** The proxy it is actually on, once assigned or pinned. */
  exit: {
    /** The proxy's id. */
    id: string;
    /** Its display name. */
    label: string;
    /** Its country and region. */
    geo: string | null;
    /** Whether its last check passed. */
    healthy: boolean;
  } | null;
  /** The device choices it was created with. */
  prefs: PersonaPrefs | null;
  /** The device it presents. */
  fingerprint: Fingerprint;
  /** Its persona-wide second factor. */
  mfa: {
    /** Whether a factor is stored. */
    configured: boolean;
    /** Which kind of factor. */
    type?: string;
    /** The site it is filed against, if any. */
    domain?: string;
  };
  /** Per-site factors and stored logins. Usernames and types only, never secrets. */
  sites: {
    /** Second factors filed against one site. */
    mfa: {
      /** The site. */
      domain: string;
      /** Which kind of factor. */
      type: string;
    }[];
    /** Stored site logins. */
    credentials: {
      /** The site. */
      domain: string;
      /** The account name. */
      username: string;
    }[];
  };
  /** What its cookie jar holds. */
  login: {
    /** Cookies in the jar. */
    cookies: number;
    /** Sites it holds a session for. */
    sites: string[];
    /** When the jar last changed. */
    updatedAt: string | null;
  };
  /** When it was created. */
  createdAt: string;
  /** When a browser last ran as it. */
  lastUsedAt: string | null;
}

/**
 * A second factor. `domain` files it against one site, because a persona driving
 * several portals meets several kinds of factor; without it the record is the
 * persona-wide default.
 *
 * `gmail` and `graph` read the code straight out of a mailbox. `email` and `sms`
 * poll an endpoint you host, set `x-oya-received-at` on its response (epoch ms)
 * and a code from a previous run will never be reused.
 *
 * The code is pulled out of the message by your own configured LLM, because
 * portals rewrite these templates constantly and the code is not always digits.
 * `pattern` is only the fallback for when no LLM key is set or the call fails.
 */
export type MfaConfig = {
  /** The site this factor answers for; without it, the persona-wide default. */
  domain?: string;
} & (
  | {
      /** An authenticator app's time-based codes. */
      type: 'totp';
      /** The base32 seed. */
      secret: string;
    }
  | {
      /** Codes delivered to an endpoint you host. */
      type: 'email' | 'sms';
      /** The endpoint polled for the latest message. */
      url: string;
      /** Headers sent with each poll. */
      headers?: Record<string, string>;
      /** Fallback regex for the code, used when no LLM is available. */
      pattern?: string;
      /** How long to wait for a code. */
      timeoutMs?: number;
    }
  | {
      /** Codes read from a Gmail or Microsoft 365 mailbox. */
      type: 'gmail' | 'graph';
      /** The mailbox's OAuth refresh token. */
      refreshToken: string;
      /** The OAuth client the token was issued to. */
      clientId: string;
      /** That client's secret, when it has one. */
      clientSecret?: string;
      /** The Microsoft tenant, for `graph`. */
      tenant?: string;
      /** A mailbox search narrowing which messages are read. */
      query?: string;
      /** Fallback regex for the code, used when no LLM is available. */
      pattern?: string;
      /** How long to wait for a code. */
      timeoutMs?: number;
    }
);

/** A site login. The password is write-only: no API ever reads it back. */
export interface SiteCredentials {
  /** The site it signs in to. */
  domain: string;
  /** The account name. */
  username: string;
  /** The password; stored sealed and never returned. */
  password: string;
}
