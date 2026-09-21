/**
 * Cookie sync with the server's pool: a full dump on connect, pulls per host
 * before navigating, and local changes forwarded in batches.
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

  /** `session()` is the persona's Electron session; `send` writes to the control socket. */
  constructor({ session, send, open, ready }) {
    Object.assign(this, { session, send, open, ready });
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

  /** Apply a full cookie jar from the server. */
  async applyCookieSync(cookies) {
    if (!Array.isArray(cookies)) return;
    this.applying = true;
    const jar = this.session().cookies;
    // One at a time meant a round trip per cookie, and auth_ok awaits this from
    // inside the serialized message queue, a real jar froze the app on sign-in.
    const results = await Promise.allSettled(cookies.map(async (c) => jar.set(electronCookie(c))));
    const applied = results.filter((r) => r.status === 'fulfilled').length;
    this.applying = false;
    console.log(`[oya] Cookie sync applied: ${applied}/${cookies.length}`);
  }

  /** Fetch this host's cookies from the pool before navigating to it. */
  pullCookiesFor(url, { force = false } = {}) {
    const host = cookieHostFor(url);
    if (!host || !this.open() || !this.ready()) return Promise.resolve();
    if (!force && Date.now() - (this.pulledAt.get(host) || 0) < COOKIE_PULL_TTL_MS) return Promise.resolve();
    if (this.pulledAt.size > COOKIE_PULLED_HOSTS_MAX) this.pulledAt.clear();
    this.pulledAt.set(host, Date.now());
    return new Promise((resolve) => this.requestPull(host, `p${++this.pullSeq}`, resolve));
  }

  /** Sends one cookie_pull and settles `resolve` on its answer or its timeout. */
  requestPull(host, pullId, resolve) {
    const pull = { host, resolve, settled: false };
    const finish = (answered = false) => this.settlePull(pullId, pull, answered);
    pull.timer = setTimeout(() => finish(false), COOKIE_PULL_TIMEOUT_MS);
    this.pendingPulls.set(pullId, finish);
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

  /** Sends the queued local changes, if the socket is up; they are dropped otherwise. */
  flushCookieChanges() {
    this.flushTimer = null;
    if (!this.batch.size) return;
    const changes = [...this.batch.values()];
    this.batch = new Map();
    if (!this.open() || !this.ready()) return;
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
    // Only forward explicit changes, ignore overwrite (intermediate removal
    // when a cookie is replaced), expired, and evicted events to prevent
    // feedback loops between browsers in the pool.
    if (cause !== 'explicit') return;
    // Keyed so a cookie rewritten repeatedly inside one window collapses to
    // its final value instead of sending every intermediate step.
    this.queueChange(`${cookie.domain}|${cookie.path}|${cookie.name}`, { removed, cookie: slimCookie(cookie) });
  }

  /** Queues one change; a full batch goes at once, otherwise on the flush timer. */
  queueChange(key, change) {
    this.batch.set(key, change);
    if (this.batch.size >= COOKIE_BATCH_MAX) {
      clearTimeout(this.flushTimer);
      this.flushCookieChanges();
      return;
    }
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
  api.forgetPulls = () => sync.pulledAt.clear();
  /** The server answered a cookie_pull. */
  api.answerPull = (pullId) => sync.pendingPulls.get(pullId)?.(true);
  return api;
}

module.exports = { createCookieSync };
