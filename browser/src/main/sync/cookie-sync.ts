/**
 * Cookie sync with the server's pool: a full dump on connect, pulls per host
 * before navigating, and local changes forwarded in batches.
 *
 * Freshness decides who wins. The server stamps each cookie with its clock when
 * the cookie really changes (`t`), and says what its clock reads (`now`) with
 * every jar it sends. This browser remembers the `now` it last synced at, and
 * takes a server cookie only if this jar lacks it or the server changed it since.
 * Without that, the server's copy overwrote this jar on every reconnect and every
 * pull, so a login made during a network blip, or a moment before a redirect to
 * a sibling host (Google's accounts → mail), was replaced by the stale session
 * it had just superseded: "my login does not stick".
 *
 * The server no longer pushes every cookie change to every browser in the pool,
 * that was O(pool size) per change. Instead each browser asks for the hosts
 * it is about to visit, so sync cost tracks navigations rather than the square
 * of the fleet size. Outbound changes are batched for the same reason.
 */
import type { Cookie, Cookies, CookiesSetDetails, Session } from 'electron';
import type { AppServices } from '../app/services.ts';
import {
  COOKIE_PULL_TTL_MS,
  COOKIE_PULL_TIMEOUT_MS,
  COOKIE_FLUSH_MS,
  COOKIE_BATCH_MAX,
  COOKIE_PULLED_HOSTS_MAX,
} from './constants.ts';

/** A cookie as the server's pool keeps and sends it. */
export interface ServerCookie {
  /** The cookie's name. */
  name: string;
  /** Its value. */
  value: string;
  /** Its domain; a leading dot makes it a domain cookie. */
  domain: string;
  /** Its path. */
  path?: string;
  /** Sent over HTTPS only. */
  secure?: boolean;
  /** Hidden from page scripts. */
  httpOnly?: boolean;
  /** The server's sameSite spelling (Strict, Lax, None) or Electron's. */
  sameSite?: string;
  /** Sent only to the exact host. */
  hostOnly?: boolean;
  /** Expiry in seconds. */
  expirationDate?: number;
  /** Expiry in seconds, as some senders spell it. */
  expires?: number;
  /** The server's clock when the cookie last really changed. */
  t?: number;
}

/** The server's clock at the last full sync, kept per persona by whoever holds it. */
export interface SyncMark {
  /** The clock reading, or 0 before any sync. */
  get(): number;
  /** Records a new reading. */
  set(now: number): void;
}

/** What the sync needs: the persona's session, the control socket, and the sync mark. */
export interface CookieSyncDeps {
  /** The active persona's Electron session (only its cookie store is used). */
  session: () => Pick<Session, 'cookies'>;
  /** Writes a message to the control socket; false when it could not. */
  send: (message: object) => unknown;
  /** Whether the socket is open. */
  open: () => boolean;
  /** Whether the socket is signed in. */
  ready: () => boolean;
  /** The server's clock at the last full sync. */
  mark?: SyncMark;
}

/** One local change, as it goes to the server. */
interface Change {
  /** True when the cookie was removed. */
  removed: boolean;
  /** The cookie, slimmed. */
  cookie: Record<string, unknown>;
}

/** A cookie_pull waiting for its answer. */
interface Pull {
  /** The host it asked for. */
  host: string;
  /** Lets the navigation that asked go on. */
  resolve: () => void;
  /** Gives up on a server that never answers. */
  timer?: ReturnType<typeof setTimeout>;
}

/** The options a jar from the server arrives with. */
interface SyncOptions {
  /** The server's clock as it sent the jar. */
  now?: number;
  /** The pull this jar answers, when it answers one. */
  pullId?: string;
}

/** Listens for changes in a cookie store. */
type ChangedListener = (event: unknown, cookie: Cookie, cause: string, removed: boolean) => void;

/** Electron's sameSite spelling for the server's. */
const SAME_SITE: Record<string, CookiesSetDetails['sameSite']> = {
  Strict: 'strict',
  Lax: 'lax',
  None: 'no_restriction',
};

/** The fields of an Electron cookie the server keeps, in the order it gets them. */
const SLIM_FIELDS = [
  'name',
  'value',
  'domain',
  'path',
  'secure',
  'httpOnly',
  'sameSite',
  'expirationDate',
  'hostOnly',
] as const;

/** The fields of an Electron cookie the server keeps. */
function slimCookie(c: Cookie): Record<string, unknown> {
  const slim: Record<string, unknown> = Object.fromEntries(SLIM_FIELDS.map((field) => [field, c[field]]));
  slim.sameSite = c.sameSite || 'unspecified';
  return slim;
}

/** A server cookie as the argument to Electron's cookies.set(). */
function electronCookie(c: ServerCookie): CookiesSetDetails {
  return { ...cookieTarget(c), ...cookieFlags(c) };
}

/** Where the cookie lives: its URL, name, value and, for a domain cookie, the domain. */
function cookieTarget(c: ServerCookie): CookiesSetDetails {
  return {
    url: `http${c.secure ? 's' : ''}://${c.domain.replace(/^\./, '')}${c.path || '/'}`,
    name: c.name,
    value: c.value,
    ...(c.hostOnly || !c.domain.startsWith('.') ? {} : { domain: c.domain }),
  };
}

/** The cookie's path, flags and expiry, in Electron's spelling. */
function cookieFlags(c: ServerCookie): Omit<CookiesSetDetails, 'url'> {
  const sameSite = Object.hasOwn(SAME_SITE, c.sameSite ?? '') && SAME_SITE[c.sameSite as string];
  return {
    path: c.path || '/',
    secure: c.secure || false,
    httpOnly: c.httpOnly || false,
    sameSite: (sameSite || c.sameSite || 'unspecified') as CookiesSetDetails['sameSite'],
    expirationDate: cookieExpiry(c),
  };
}

/** Expiry in seconds (the server may say `expires`), or undefined for a session cookie. */
function cookieExpiry(c: ServerCookie): number | undefined {
  const expires = Number(c.expirationDate ?? c.expires);
  return expires > 0 ? expires : undefined;
}

/** A cookie's identity, the same for Electron's cookie and the server's copy of it. */
const keyOf = (c: Pick<Cookie, 'domain' | 'path' | 'name'>): string => `${c.domain}|${c.path || '/'}|${c.name}`;

/**
 * Changes to the jar worth telling the server: a cookie set or deleted, a session
 * refreshed with a new expiry, and a site expiring a cookie (how a logout deletes
 * one). Electron named a set cookie "explicit" until 36 and "inserted" since, so
 * both are here: an upgrade that kept only the old name stopped every sync, silently.
 */
const FORWARDED_CAUSES = ['explicit', 'inserted', 'inserted-no-value-change-overwrite', 'expired-overwrite'];

/** A sync mark for a caller that keeps none: never synced, nothing remembered. */
const NO_MARK: SyncMark = { get: () => 0, set: () => {} };

/** Await every native write, but never report a partially restored login as synchronized. */
async function writeJar(jar: Cookies, cookies: ServerCookie[], offered: number): Promise<void> {
  const results = await Promise.allSettled(cookies.map(async (c) => jar.set(electronCookie(c))));
  const applied = results.filter((r) => r.status === 'fulfilled').length;
  console.log(`[oya] Cookie sync applied: ${applied}/${offered}`);
  if (applied !== cookies.length)
    throw new Error('Cookie sync incomplete: the native cookie store rejected saved cookies');
}

/** The hostname a navigation to `url` will reach, or null. */
function cookieHostFor(url: string): string | null {
  return URL.parse(/^https?:\/\//i.test(url) ? url : 'https://' + url)?.hostname ?? null;
}

/** One persona's cookie traffic with the server. Wired in as ctx.cookies. */
export class CookieSync {
  /** The persona's session, the socket and the sync mark. */
  private readonly deps: Required<CookieSyncDeps>;
  /** True while a server jar is being written, so those writes are not echoed back. */
  private applying = false;
  /** Host → when it was last pulled. */
  private readonly pulledAt = new Map<string, number>();
  /** Pull id → the pull waiting for its answer. */
  private readonly pendingPulls = new Map<string, Pull>();
  /** Last pull id issued. */
  private pullSeq = 0;
  /** Queued local changes, keyed by domain|path|name. */
  private batch = new Map<string, Change>();
  /** Timer that sends the queued batch. */
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  /** The cookie store being watched. */
  private watched: Cookies | null = null;
  /** The listener on `watched`. */
  private listener: ChangedListener | null = null;
  /** When this jar last went to the server, whole or as a batch of changes (0 before it ever has). */
  private sentAt = 0;
  /** Host → the server's clock when that host was last pulled. */
  private readonly hostMarks = new Map<string, number>();

  /** `session()` is the persona's Electron session; `send` writes to the control socket; `mark` keeps the server's clock at the last full sync. */
  constructor({ mark = NO_MARK, ...deps }: CookieSyncDeps) {
    this.deps = { ...deps, mark };
  }

  /** Dump all cookies to the server for pool sync. */
  async dumpCookies(): Promise<void> {
    if (!this.deps.open()) return;
    try {
      const cookies = await this.deps.session().cookies.get({});
      this.deps.send({ type: 'cookie_dump', cookies: cookies.map(slimCookie) });
      this.sentAt = Date.now();
    } catch (e) {
      console.log('[oya] Cookie dump failed:', (e as Error).message);
    }
  }

  /**
   * Applies a jar from the server: the full one on connect, or one host's on a
   * pull (`pullId`). `now` is the server's clock as it sent it.
   */
  async applyCookieSync(cookies: unknown, { now, pullId }: SyncOptions = {}): Promise<void> {
    if (!Array.isArray(cookies)) return;
    const host = pullId === undefined ? undefined : this.pendingPulls.get(pullId)?.host;
    const since = (host === undefined ? undefined : this.hostMarks.get(host)) ?? this.deps.mark.get();
    const fresh = await this.fresherThanLocal(cookies, since);
    await this.writeCookies(fresh, cookies.length);
    if (now) this.markSynced(now, host);
  }

  /** The server's cookies this jar should take: ones it lacks, and ones the server changed after `since`; never one with a local change still unsent. */
  private async fresherThanLocal(cookies: ServerCookie[], since: number): Promise<ServerCookie[]> {
    const local = new Set((await this.deps.session().cookies.get({})).map(keyOf));
    const newer = (c: ServerCookie) => !local.has(keyOf(c)) || (c.t || 0) > since;
    return cookies.filter((c) => !this.batch.has(keyOf(c)) && newer(c));
  }

  /** Writes cookies to the jar without echoing them back to the server. */
  private async writeCookies(cookies: ServerCookie[], offered: number): Promise<void> {
    this.applying = true;
    try {
      await writeJar(this.deps.session().cookies, cookies, offered);
    } finally {
      this.applying = false;
    }
  }

  /** In step with the server as of `now`: for one pulled host, or (the full jar) for everything. */
  private markSynced(now: number, host: string | undefined): void {
    if (host) return void this.hostMarks.set(host, now);
    this.hostMarks.clear();
    this.deps.mark.set(now);
  }

  /** When this jar last went to the server (0 before it ever has). */
  syncedAt(): number {
    return this.sentAt;
  }

  /** Forget which hosts were pulled: the jar just changed persona. */
  forgetPulls(): void {
    this.pulledAt.clear();
    this.hostMarks.clear();
  }

  /** Fetch this host's cookies from the pool before navigating to it. */
  pullCookiesFor(url: string, { force = false } = {}): Promise<void> {
    const host = cookieHostFor(url);
    if (!host || !this.deps.open() || !this.deps.ready()) return Promise.resolve();
    if (!force && Date.now() - (this.pulledAt.get(host) || 0) < COOKIE_PULL_TTL_MS) return Promise.resolve();
    if (this.pulledAt.size > COOKIE_PULLED_HOSTS_MAX) this.pulledAt.clear();
    this.pulledAt.set(host, Date.now());
    // Ahead of the pull on the same socket, so the answer is never older than this jar.
    this.flushCookieChanges();
    return new Promise((resolve) => this.requestPull(host, `p${++this.pullSeq}`, resolve));
  }

  /** Sends one cookie_pull and settles `resolve` on its answer or its timeout. */
  private requestPull(host: string, pullId: string, resolve: () => void): void {
    const pull: Pull = { host, resolve };
    pull.timer = setTimeout(() => this.settlePull(pullId, false), COOKIE_PULL_TIMEOUT_MS);
    this.pendingPulls.set(pullId, pull);
    if (!this.trySend({ type: 'cookie_pull', domains: [host], pullId })) this.settlePull(pullId, false);
  }

  /** The server answered a cookie_pull; a jar it sent unasked (no pull id) settles nothing. */
  answerPull(pullId: string | undefined): void {
    if (pullId !== undefined) this.settlePull(pullId, true);
  }

  /** Sends, reporting a throw as a failed send. */
  private trySend(message: object): unknown {
    try {
      return this.deps.send(message);
    } catch {
      return false;
    }
  }

  /**
   * Settles one pull, once. The eager pulledAt.set in pullCookiesFor dedupes
   * concurrent navigations to this host. Only a real answer may keep it: a pull
   * that timed out synced nothing, and leaving the stamp in place suppressed
   * every retry for the whole TTL.
   */
  private settlePull(pullId: string, answered: boolean): void {
    const pull = this.pendingPulls.get(pullId);
    if (!pull) return;
    clearTimeout(pull.timer);
    this.pendingPulls.delete(pullId);
    if (answered) this.pulledAt.set(pull.host, Date.now());
    else this.pulledAt.delete(pull.host);
    pull.resolve();
  }

  /** Forget queued changes without sending them: used when the persona changes. */
  dropPendingCookieChanges(): void {
    this.stopFlushTimer();
    this.batch = new Map();
  }

  /**
   * Sends the queued local changes. With the socket down they wait for it: they
   * used to be dropped, and the reconnect then put the server's older copies back
   * over the very cookies that had changed.
   */
  flushCookieChanges(): void {
    this.stopFlushTimer();
    if (!this.batch.size || !this.deps.open() || !this.deps.ready()) return;
    const changes = [...this.batch.values()];
    this.batch = new Map();
    this.deps.send({ type: 'cookie_changed', changes });
    this.sentAt = Date.now();
  }

  /** Cancels the pending batch send, if any. */
  private stopFlushTimer(): void {
    if (this.flushTimer) clearTimeout(this.flushTimer);
    this.flushTimer = null;
  }

  /** Start listening for local cookie changes and forward them in batches. */
  startCookieChangeListener(): void {
    if (this.watched && this.listener) this.watched.removeListener('changed', this.listener);
    const watched = this.deps.session().cookies;
    const listener: ChangedListener = (_event, cookie, cause, removed) => this.cookieChanged(cookie, cause, removed);
    watched.on('changed', listener);
    this.watched = watched;
    this.listener = listener;
  }

  /** One change in the local jar. */
  private cookieChanged(cookie: Cookie, cause: string, removed: boolean): void {
    if (this.applying) return;
    // Ignore overwrite (the intermediate removal when a cookie is replaced),
    // expired and evicted events, to prevent feedback loops between browsers.
    if (!FORWARDED_CAUSES.includes(cause)) return;
    // Keyed so a cookie rewritten repeatedly inside one window collapses to
    // its final value instead of sending every intermediate step.
    this.queueChange(keyOf(cookie), { removed, cookie: slimCookie(cookie) });
  }

  /** Queues one change; a full batch goes at once, otherwise on the flush timer. */
  private queueChange(key: string, change: Change): void {
    this.batch.set(key, change);
    if (this.batch.size >= COOKIE_BATCH_MAX) return this.flushCookieChanges();
    if (!this.flushTimer) this.flushTimer = setTimeout(() => this.flushCookieChanges(), COOKIE_FLUSH_MS);
  }
}

/** The sync mark of whichever persona is active, kept in the app's config so it outlives a restart. */
export function cookieSyncMark(ctx: Pick<AppServices, 'config' | 'persona'>): SyncMark {
  const marks = (): Record<string, number> =>
    ((ctx.config.values.cookieSyncedAt as Record<string, number> | undefined) ||= {});
  const id = (): string => ctx.persona.active?.id || '';
  const set = (now: number) => ((marks()[id()] = now), ctx.config.save());
  return { get: () => marks()[id()] || 0, set };
}
