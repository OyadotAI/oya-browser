/**
 * The shapes the client's namespaces (`oya.browser`, `oya.control`,
 * `oya.personas` …) take and return that are not part of the exported types,
 * named so each field can say what it is.
 */
import type { Http } from '../client.js';
import type {
  BrowserInfo,
  ControlCredential,
  ControlEvent,
  ControlRole,
  Cookie,
  Fingerprint,
  PersonaInfo,
  PersonaPrefs,
  PlaybookSummary,
  ProxyInfo,
  StopResult,
} from '../types/index.js';

/** The client's Http, read when a call is made: the namespaces are built before the constructor sets it. */
export type HttpRef = () => Http;

/** A browser as `GET /api/browsers/:id` answers, with where to point a CDP client. */
export interface ConnectedBrowser extends BrowserInfo {
  /** Point Playwright, Puppeteer or browser-use at this. */
  cdpUrl?: string;
}

/** What stopping several browsers did. */
export interface StopManyResult {
  /** How many stopped. */
  stopped: number;
  /** Each browser's outcome. */
  results: StopResult[];
}

/** A plain acknowledgement. */
export interface Ack {
  /** Whether it was done. */
  ok: boolean;
}

/** A single-use live-stream ticket. */
export interface Ticket {
  /** The ticket. */
  ticket: string;
  /** Seconds until it expires. */
  expiresIn: number;
}

/** A page of lifecycle events. */
export interface EventPage {
  /** Events after the cursor asked for. */
  events: ControlEvent[];
  /** Pass this as `after` to read on. */
  cursor: number;
}

/** A service credential to mint. */
export interface CredentialRequest {
  /** What it may do. */
  role: ControlRole;
  /** What it is for. */
  label?: string;
  /** When it stops working, in epoch milliseconds. */
  expiresAt?: number;
}

/** A new service credential, with its token: shown this once. */
export interface NewCredential extends ControlCredential {
  /** The secret to authenticate with. */
  token: string;
}

/** One project member. */
export interface Member {
  /** The member's user id. */
  userId: string;
  /** What they may do. */
  role: ControlRole;
}

/** The project's owner and members. */
export interface MemberList {
  /** The owner's user id. */
  owner: string | null;
  /** Everyone else. */
  members: Member[];
}

/** An invitation code. */
export interface Invite {
  /** The code to hand to the invitee. */
  code: string;
  /** Seconds until it expires. */
  expiresIn: number;
}

/** A registered webhook. */
export interface Webhook {
  /** Its id, for removing it. */
  id: string;
  /** The key its deliveries are signed with. */
  secret: string;
}

/** One proxy's check. */
export interface ProxyCheck {
  /** The proxy. */
  id: string;
  /** Whether it answered. */
  ok: boolean;
  /** Where its traffic actually leaves. */
  exitIp?: string | null;
  /** Why it failed. */
  error?: string;
}

/** Where a persona's proxy should be. */
export interface GeoHint {
  /** Two-letter country, optionally a region. */
  geo?: string;
}

/** A persona to create. */
export interface PersonaCreate {
  /** Its display name. */
  name?: string;
  /** Device choices, fixed for its life. */
  prefs?: PersonaPrefs;
  /** Where its proxy should be. */
  proxy?: GeoHint;
  /** How many browsers may run as it at once; null for no cap. */
  maxConcurrent?: number | null;
}

/** What may change on a persona. Never the device. */
export interface PersonaUpdate {
  /** Its display name. */
  name?: string;
  /** How many browsers may run as it at once; null for no cap. */
  maxConcurrent?: number | null;
  /** Where its proxy should be; null clears the hint. */
  proxy?: GeoHint | null;
}

/** Options for a clone. */
export interface CloneOptions {
  /** The new persona's name. */
  name?: string;
}

/** What a persona may claim, per platform. */
export interface PersonaOptions {
  /** The platforms on offer. */
  platforms: string[];
  /** Timezones each platform may coherently claim. */
  timezones: Record<string, string[]>;
  /** Locales each platform may coherently claim. */
  locales: Record<string, string[]>;
}

/** The proxy a persona is pinned to. */
export interface PinnedProxy {
  /** The proxy's id. */
  id: string;
  /** Its display name. */
  label: string;
}

/** The outcome of pinning a proxy. */
export interface ProxyPin {
  /** Whether it was saved. */
  ok: boolean;
  /** The proxy now pinned, or null when assignment is left to connect time. */
  proxy: PinnedProxy | null;
}

/** A stored second factor, as confirmed. */
export interface MfaSaved {
  /** Whether it is stored. */
  configured: boolean;
  /** Which kind of factor. */
  type: string;
}

/** A site login the persona can use. Never the password. */
export interface SiteLogin {
  /** The site. */
  domain: string;
  /** The account name. */
  username: string;
}

/** A stored site login, as confirmed. */
export interface CredentialsSaved extends SiteLogin {
  /** Whether it is stored. */
  configured: boolean;
}

/** The sites a persona can sign in to. */
export interface SiteLogins {
  /** Usernames only. */
  credentials: SiteLogin[];
}

/** `GET /api/pool/cookies`. */
export interface CookieJar {
  /** The persona the jar belongs to. */
  persona: string;
  /** Its cookies. */
  cookies: Cookie[];
}

/** `PUT /api/pool/cookies`: what an import did. */
export interface CookiesImported {
  /** The persona the cookies went to. */
  persona: string;
  /** Cookies merged into the jar. */
  imported: number;
  /** Cookies left out: no name, value or domain, or already expired. */
  skipped: number;
  /** Cookies in the jar afterwards. */
  total: number;
}

/** `GET /api/playbooks`. */
export interface PlaybookList {
  /** Every saved playbook. */
  playbooks: PlaybookSummary[];
}

/** `GET /api/proxies`. */
export interface ProxyList {
  /** Every proxy this key can use. */
  proxies: ProxyInfo[];
}

/** `POST /api/proxies/check`. */
export interface ProxyCheckList {
  /** Each proxy's check. */
  results: ProxyCheck[];
}

/** `GET /api/personas`. */
export interface PersonaList {
  /** Every persona on this key. */
  personas: PersonaInfo[];
}

/** `POST /api/personas/preview`. */
export interface FingerprintPreview {
  /** The device these choices would produce. */
  fingerprint: Fingerprint;
}
