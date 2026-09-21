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
const {
  COOKIE_PULL_TTL_MS,
  COOKIE_PULL_TIMEOUT_MS,
  COOKIE_FLUSH_MS,
  COOKIE_BATCH_MAX,
  COOKIE_PULLED_HOSTS_MAX,
} = require('./constants.cjs');

/** Electron's sameSite spelling for the server's. */
const SAME_SITE = { Strict: 'strict', Lax: 'lax', None: 'no_restriction' };

/** The fields of an Electron cookie the server keeps, in the order it gets them. */
const SLIM_FIELDS = ['name', 'value', 'domain', 'path', 'secure', 'httpOnly', 'sameSite', 'expirationDate', 'hostOnly'];

/** The fields of an Electron cookie the server keeps. */
function slimCookie(c) {
  const slim = Object.fromEntries(SLIM_FIELDS.map((field) => [field, c[field]]));
  slim.sameSite = c.sameSite || 'unspecified';
  return slim;
}

/** A server cookie as the argument to Electron's cookies.set(). */
function electronCookie(c) {
  return { ...cookieTarget(c), ...cookieFlags(c) };
}

/** Where the cookie lives: its URL, name, value and, for a domain cookie, the domain. */
function cookieTarget(c) {
  return {
    url: `http${c.secure ? 's' : ''}://${c.domain.replace(/^\./, '')}${c.path || '/'}`,
    name: c.name,
    value: c.value,
    ...(c.hostOnly || !c.domain.startsWith('.') ? {} : { domain: c.domain }),
  };
}

/** The cookie's path, flags and expiry, in Electron's spelling. */
function cookieFlags(c) {
  const sameSite = Object.hasOwn(SAME_SITE, c.sameSite ?? '') && SAME_SITE[c.sameSite];
  return {
    path: c.path || '/',
    secure: c.secure || false,
    httpOnly: c.httpOnly || false,
    sameSite: sameSite || c.sameSite || 'unspecified',
    expirationDate: cookieExpiry(c),
  };
}

/** Expiry in seconds (the server may say `expires`), or undefined for a session cookie. */
function cookieExpiry(c) {
  const expires = Number(c.expirationDate ?? c.expires);
  return expires > 0 ? expires : undefined;
}

/** A cookie's identity, the same for Electron's cookie and the server's copy of it. */
const keyOf = (c) => `${c.domain}|${c.path || '/'}|${c.name}`;

/**
 * Changes to the jar worth telling the server: a cookie set or deleted, a session
 * refreshed with a new expiry, and a site expiring a cookie (how a logout deletes
 * one). Electron named a set cookie "explicit" until 36 and "inserted" since, so
 * both are here: an upgrade that kept only the old name stopped every sync, silently.
 */
const FORWARDED_CAUSES = ['explicit', 'inserted', 'inserted-no-value-change-overwrite', 'expired-overwrite'];

/** A sync mark for a caller that keeps none: never synced, nothing remembered. */
const NO_MARK = { get: () => 0, set: () => {} };

/** The hostname a navigation to `url` will reach, or null. */
function cookieHostFor(url) {
  return URL.parse(/^https?:\/\//i.test(url) ? url : 'https://' + url)?.hostname ?? null;
}

/** One persona's cookie traffic with the server. */
class CookieSync {
  /** True while a server jar is being written, so those writes are not echoed back. */
  applying = false;
  /** Host → when it was last pulled. */
  pulledAt = new Map();
  /** Pull id → the function that settles its wait. */
  pendingPulls = new Map();
  /** Last pull id issued. */
  pullSeq = 0;
  /** Queued local changes, keyed by domain|path|name. */
  batch = new Map();
  /** Timer that sends the queued batch. */
  flushTimer = null;
  /** The cookie store being watched. */
  watched = null;
  /** The listener on `watched`. */
  listener = null;

  /** Host → the server's clock when that host was last pulled. */
  hostMarks = new Map();

  /** `session()` is the persona's Electron session; `send` writes to the control socket; `mark` keeps the server's clock at the last full sync. */
  constructor({ session, send, open, ready, mark = NO_MARK }) {
    Object.assign(this, { session, send, open, ready, mark });
  }

  /** Dump all cookies to the server for pool sync. */
  async dumpCookies() {
    if (!this.open()) return;
    try {
      const cookies = await this.session().cookies.get({});
      this.send({ type: 'cookie_dump', cookies: cookies.map(slimCookie) });
    } catch (e) {
      console.log('[oya] Cookie dump failed:', e.message);
    }
  }

  /**
   * Applies a jar from the server: the full one on connect, or one host's on a
   * pull (`pullId`). `now` is the server's clock as it sent it.
   */
  async applyCookieSync(cookies, { now, pullId } = {}) {
    if (!Array.isArray(cookies)) return;
    const host = this.pendingPulls.get(pullId)?.host;
    const fresh = await this.fresherThanLocal(cookies, this.hostMarks.get(host) ?? this.mark.get());
    await this.writeCookies(fresh, cookies.length);
    if (now) this.syncedAt(now, host);
  }

  /** The server's cookies this jar should take: ones it lacks, and ones the server changed after `since`; never one with a local change still unsent. */
  async fresherThanLocal(cookies, since) {
    const local = new Set((await this.session().cookies.get({})).map(keyOf));
    const newer = (c) => !local.has(keyOf(c)) || (c.t || 0) > since;
    return cookies.filter((c) => !this.batch.has(keyOf(c)) && newer(c));
  }

  /** Writes cookies to the jar without echoing them back to the server. */
  async writeCookies(cookies, offered) {
    this.applying = true;
    const jar = this.session().cookies;
    // One at a time meant a round trip per cookie, and auth_ok awaits this from
    // inside the serialized message queue, a real jar froze the app on sign-in.
    const results = await Promise.allSettled(cookies.map(async (c) => jar.set(electronCookie(c))));
    const applied = results.filter((r) => r.status === 'fulfilled').length;
    this.applying = false;
    console.log(`[oya] Cookie sync applied: ${applied}/${offered}`);
  }

  /** In step with the server as of `now`: for one pulled host, or (the full jar) for everything. */
  syncedAt(now, host) {
    if (host) return void this.hostMarks.set(host, now);
    this.hostMarks.clear();
    this.mark.set(now);
  }

  /** Fetch this host's cookies from the pool before navigating to it. */
  pullCookiesFor(url, { force = false } = {}) {
    const host = cookieHostFor(url);
    if (!host || !this.open() || !this.ready()) return Promise.resolve();
    if (!force && Date.now() - (this.pulledAt.get(host) || 0) < COOKIE_PULL_TTL_MS) return Promise.resolve();
    if (this.pulledAt.size > COOKIE_PULLED_HOSTS_MAX) this.pulledAt.clear();
    this.pulledAt.set(host, Date.now());
    // Ahead of the pull on the same socket, so the answer is never older than this jar.
    this.flushCookieChanges();
    return new Promise((resolve) => this.requestPull(host, `p${++this.pullSeq}`, resolve));
  }

  /** Sends one cookie_pull and settles `resolve` on its answer or its timeout. */
  requestPull(host, pullId, resolve) {
    const pull = { host, resolve, settled: false };
    const finish = (answered = false) => this.settlePull(pullId, pull, answered);
    pull.timer = setTimeout(() => finish(false), COOKIE_PULL_TIMEOUT_MS);
    this.pendingPulls.set(pullId, Object.assign(finish, { host }));
    if (!this.trySend({ type: 'cookie_pull', domains: [host], pullId })) finish();
  }

  /** Sends, reporting a throw as a failed send. */
  trySend(message) {
    try {
      return this.send(message);
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
  settlePull(pullId, pull, answered) {
    if (pull.settled) return;
    pull.settled = true;
    clearTimeout(pull.timer);
    this.pendingPulls.delete(pullId);
    if (answered) this.pulledAt.set(pull.host, Date.now());
    else this.pulledAt.delete(pull.host);
    pull.resolve();
  }

  /** Forget queued changes without sending them: used when the persona changes. */
  dropPendingCookieChanges() {
    clearTimeout(this.flushTimer);
    this.flushTimer = null;
    this.batch = new Map();
  }

  /**
   * Sends the queued local changes. With the socket down they wait for it: they
   * used to be dropped, and the reconnect then put the server's older copies back
   * over the very cookies that had changed.
   */
  flushCookieChanges() {
    clearTimeout(this.flushTimer);
    this.flushTimer = null;
    if (!this.batch.size || !this.open() || !this.ready()) return;
    const changes = [...this.batch.values()];
    this.batch = new Map();
    this.send({ type: 'cookie_changed', changes });
  }

  /** Start listening for local cookie changes and forward them in batches. */
  startCookieChangeListener() {
    if (this.watched && this.listener) this.watched.removeListener('changed', this.listener);
    this.watched = this.session().cookies;
    this.listener = (_event, cookie, cause, removed) => this.cookieChanged(cookie, cause, removed);
    this.watched.on('changed', this.listener);
  }

  /** One change in the local jar. */
  cookieChanged(cookie, cause, removed) {
    if (this.applying) return;
    // Ignore overwrite (the intermediate removal when a cookie is replaced),
    // expired and evicted events, to prevent feedback loops between browsers.
    if (!FORWARDED_CAUSES.includes(cause)) return;
    // Keyed so a cookie rewritten repeatedly inside one window collapses to
    // its final value instead of sending every intermediate step.
    this.queueChange(keyOf(cookie), { removed, cookie: slimCookie(cookie) });
  }

  /** Queues one change; a full batch goes at once, otherwise on the flush timer. */
  queueChange(key, change) {
    this.batch.set(key, change);
    if (this.batch.size >= COOKIE_BATCH_MAX) return this.flushCookieChanges();
    if (!this.flushTimer) this.flushTimer = setTimeout(() => this.flushCookieChanges(), COOKIE_FLUSH_MS);
  }
}

/** The sync's functions, bound, under the names main.js uses. */
const COOKIE_SYNC_API = [
  'dumpCookies',
  'applyCookieSync',
  'pullCookiesFor',
  'dropPendingCookieChanges',
  'flushCookieChanges',
  'startCookieChangeListener',
];

/** `session()` is the persona's Electron session; `send` writes to the control socket. */
function createCookieSync(deps) {
  const sync = new CookieSync(deps);
  const api = Object.fromEntries(COOKIE_SYNC_API.map((name) => [name, sync[name].bind(sync)]));
  /** Forget which hosts were pulled: the jar just changed persona. */
  api.forgetPulls = () => (sync.pulledAt.clear(), sync.hostMarks.clear());
  /** The server answered a cookie_pull. */
  api.answerPull = (pullId) => sync.pendingPulls.get(pullId)?.(true);
  return api;
}

/** The sync mark of whichever persona is active, kept in the app's config so it outlives a restart. */
function cookieSyncMark(ctx) {
  const marks = () => (ctx.config.values.cookieSyncedAt ||= {});
  const id = () => ctx.persona.active?.id || '';
  const set = (now) => ((marks()[id()] = now), ctx.config.save());
  return { get: () => marks()[id()] || 0, set };
}

module.exports = { createCookieSync, cookieSyncMark };
