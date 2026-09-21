/**
 * `oya.personas` (and its alias `oya.profiles`): identities, each one device
 * with its cookie jar and proxy, plus the second factors and site logins
 * stored for them. Built from one group per topic.
 *
 * Persona ids go into paths as they are, unencoded, as they always have.
 */
import type {
  Cookie,
  CookieFormat,
  Fingerprint,
  MfaConfig,
  PersonaInfo,
  PersonaPrefs,
  SiteCredentials,
} from '../types/index.js';
import type {
  CloneOptions,
  CookieJar,
  CookiesImported,
  CredentialsSaved,
  FingerprintPreview,
  HttpRef,
  MfaSaved,
  PersonaCreate,
  PersonaList,
  PersonaOptions,
  PersonaUpdate,
  ProxyPin,
  SiteLogins,
} from './shapes.js';

/** Finding, creating and changing personas. */
const identityCalls = (http: HttpRef) => ({
  /** Every persona on this key. */
  list: async (): Promise<PersonaInfo[]> => (await http().request<PersonaList>('GET', '/api/personas')).personas,
  /** One persona. */
  get: (id: string): Promise<PersonaInfo> => http().request<PersonaInfo>('GET', `/api/personas/${id}`),
  /**
   * Create an identity. The device, platform, timezone, locale, is chosen
   * here and fixed for its life; `preview()` shows what a choice produces.
   */
  create: (options: PersonaCreate = {}): Promise<PersonaInfo> =>
    http().request<PersonaInfo>('POST', '/api/personas', options),
  /** Name, concurrency cap and proxy hint. Never the device, clone for that. */
  update: (id: string, changes: PersonaUpdate): Promise<PersonaInfo> =>
    http().request<PersonaInfo>('PUT', `/api/personas/${id}`, changes),
  /** A new persona of the same kind of device: same choices, fresh identity, empty jar. */
  clone: (id: string, options: CloneOptions = {}): Promise<PersonaInfo> =>
    http().request<PersonaInfo>('POST', `/api/personas/${id}/clone`, options),
});

/** Device choices, proxy pinning and removal. */
const deviceCalls = (http: HttpRef) => ({
  /** The fingerprint these choices would produce. Persists nothing. */
  preview: async (prefs: PersonaPrefs = {}): Promise<Fingerprint> =>
    (await http().request<FingerprintPreview>('POST', '/api/personas/preview', { prefs })).fingerprint,
  /** Platforms, and the timezones and locales each may coherently claim. */
  options: (): Promise<PersonaOptions> => http().request('GET', '/api/personas/options'),
  /** Pin the persona to one of your proxies, or `null` to let assignment happen at connect. */
  pinProxy: (id: string, proxyId: string | null) =>
    http().request<ProxyPin>('PUT', `/api/personas/${id}/proxy`, { proxyId }),
  /** Delete a persona. */
  remove: async (id: string): Promise<void> => {
    await http().request('DELETE', `/api/personas/${id}`);
  },
});

/** Second factors. */
const mfaCalls = (http: HttpRef) => ({
  /** Store the second factor for this identity. Sealed at rest, never read back. */
  setMfa: (id: string, config: MfaConfig): Promise<MfaSaved> =>
    http().request('PUT', `/api/personas/${id}/mfa`, config),
  /** Remove the persona-wide factor, or the one filed against `domain`. */
  clearMfa: async (id: string, domain?: string): Promise<void> => {
    await http().request('DELETE', `/api/personas/${id}/mfa${domain ? `?domain=${encodeURIComponent(domain)}` : ''}`);
  },
});

/** Site logins. */
const loginCalls = (http: HttpRef) => ({
  /**
   * Store a site login for this identity. Sealed at rest, never read back.
   *
   * Signing in once on the desktop and inheriting the cookies is still the
   * better path. This is for portals that expire a session server-side
   * between runs, where an unattended run has nothing else to recover with.
   */
  setCredentials: (id: string, config: SiteCredentials): Promise<CredentialsSaved> =>
    http().request('PUT', `/api/personas/${id}/credentials`, config),
  /** Which sites this identity can sign in to. Usernames only. */
  credentials: (id: string): Promise<SiteLogins> => http().request('GET', `/api/personas/${id}/credentials`),
  /** Remove the login stored for one site. */
  clearCredentials: async (id: string, domain: string): Promise<void> => {
    await http().request('DELETE', `/api/personas/${id}/credentials?domain=${encodeURIComponent(domain)}`);
  },
});

/** The jar's path for a persona. */
const jarPath = (id: string) => `/api/pool/cookies?persona=${encodeURIComponent(id)}`;

/** Reading a jar and writing into one: what copying between personas is made of. */
const jarCalls = (http: HttpRef) => ({
  /** Every cookie in the jar; `format: 'playwright'` is ready for `context.addCookies()`. */
  cookies: async (id: string, format: CookieFormat = 'json'): Promise<Cookie[]> =>
    (await http().request<CookieJar>('GET', `${jarPath(id)}&format=${format}`)).cookies,
  /** Merge cookies into the jar. The persona's browsers pick them up on their next visit to each site. */
  importCookies: (id: string, list: Cookie[]): Promise<CookiesImported> =>
    http().request<CookiesImported>('PUT', jarPath(id), { cookies: list }),
});

/**
 * The cookie jar: the logins themselves. Sign in once, anywhere, and carry the
 * session to wherever it is needed: another persona, a script, another browser.
 */
const cookieCalls = (http: HttpRef) => ({
  ...jarCalls(http),
  /** Copy every login of persona `from` into persona `to`, which keeps its own device. */
  copyCookies: async (from: string, to: string): Promise<CookiesImported> =>
    jarCalls(http).importCookies(to, await jarCalls(http).cookies(from)),
  /** Forget every cookie in the jar: signs the persona out everywhere. */
  clearCookies: async (id: string): Promise<void> => {
    await http().request('DELETE', jarPath(id));
  },
});

/** Builds `oya.personas`. */
export const personaApi = (http: HttpRef) => ({
  ...identityCalls(http),
  ...deviceCalls(http),
  ...mfaCalls(http),
  ...loginCalls(http),
  ...cookieCalls(http),
});
