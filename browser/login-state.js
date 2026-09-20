/**
 * Shared localStorage transport for desktop and CDP browsers. No page globals.
 * The server requires this file too (drivers/cdp), and its image copies only
 * this file, so it requires nothing of its own.
 */

/** The DOMStorage events that mean an origin's localStorage changed. */
const STORAGE_EVENTS = [
  'domStorageItemAdded',
  'domStorageItemUpdated',
  'domStorageItemRemoved',
  'domStorageItemsCleared',
];

/** Chromium's sameSite spelling, from Electron's or the server's. */
const CDP_SAME_SITE = {
  strict: { sameSite: 'Strict' },
  lax: { sameSite: 'Lax' },
  no_restriction: { sameSite: 'None' },
  Strict: { sameSite: 'Strict' },
  Lax: { sameSite: 'Lax' },
  None: { sameSite: 'None' },
};

/**
 * One persona's localStorage, carried between browsers: saved values are
 * written into each origin before its scripts run, and changes are captured
 * and reported through `onChange`.
 */
class LoginState {
  /** `origins` maps an origin to its saved localStorage; `onChange` hears each captured change. */
  constructor(origins = {}, onChange = () => {}) {
    this.origins = origins;
    this.onChange = onChange;
    this.visited = new Set();
    this.pages = new Set();
  }

  /** Starts carrying storage for one page, driven by its CDP `send` and event subscription `on`. */
  async attach(send, on) {
    const page = { send, script: null, queue: Promise.resolve() };
    this.pages.add(page);
    page.refresh = () => this.refresh(page);
    this.listen(send, on);
    await send('Page.enable');
    await send('DOMStorage.enable');
    await page.refresh();
  }

  /** Follows the page's navigations and localStorage changes. */
  listen(send, on) {
    on('Page.frameNavigated', ({ frame }) => this.navigated(frame));
    const capture = (event) => this.capture(send, event);
    for (const event of STORAGE_EVENTS) on(`DOMStorage.${event}`, capture);
  }

  /** Re-installs the page's restore script with the origins not yet visited. */
  refresh(page) {
    page.queue = page.queue
      .then(() => this.installRestore(page))
      .catch(() => {
        this.pages.delete(page);
      });
    return page.queue;
  }

  /** Swaps the page's restore script for a current one. */
  async installRestore(page) {
    if (page.script) await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: page.script });
    const pending = Object.fromEntries(Object.entries(this.origins).filter(([origin]) => !this.visited.has(origin)));
    const source = `(() => { try { const entries = ${JSON.stringify(pending)}[location.origin]; if (entries) for (const [key, value] of Object.entries(entries)) localStorage.setItem(key, value); } catch {} })()`;
    const result = await page.send('Page.addScriptToEvaluateOnNewDocument', { source });
    page.script = result.identifier;
  }

  /** A top-level navigation: that origin has had its restore, so no page restores it again. */
  navigated(frame) {
    if (frame.parentId) return;
    const origin = URL.parse(frame.url)?.origin;
    if (!origin || !/^https?:/.test(origin)) return;
    this.visited.add(origin);
    for (const other of this.pages) other.refresh();
  }

  /** Reads an origin's localStorage after it changed and reports it. */
  async capture(send, { storageId }) {
    if (!storageId?.isLocalStorage || !/^https?:/.test(storageId.securityOrigin || '')) return;
    try {
      const { entries } = await send('DOMStorage.getDOMStorageItems', { storageId });
      const values = Object.fromEntries(entries);
      this.origins[storageId.securityOrigin] = values;
      this.onChange({ [storageId.securityOrigin]: values });
    } catch {
      /* A closed tab or navigating document no longer has storage. */
    }
  }
}

/** Where a cookie is set: a URL for a host-only cookie, the domain otherwise. */
function cdpCookieTarget(c) {
  return c.hostOnly || !c.domain.startsWith('.')
    ? { url: `http${c.secure ? 's' : ''}://${c.domain.replace(/^\./, '')}${c.path || '/'}` }
    : { domain: c.domain };
}

/** Drop Chromium's read-only cookie fields and translate Electron's spelling. */
function cdpCookies(cookies) {
  return cookies.map((c) => ({ ...cdpCookieBase(c), ...cdpCookieTarget(c), ...cdpCookieExtras(c) }));
}

/** Name, value, path and the two flags every cookie has. */
function cdpCookieBase(c) {
  return { name: c.name, value: c.value, path: c.path || '/', secure: !!c.secure, httpOnly: !!c.httpOnly };
}

/** The expiry, when there is one, and the sameSite Chromium understands. */
function cdpCookieExtras(c) {
  return {
    ...(Number(c.expirationDate ?? c.expires) > 0 ? { expires: Number(c.expirationDate ?? c.expires) } : {}),
    ...((Object.hasOwn(CDP_SAME_SITE, c.sameSite ?? '') && CDP_SAME_SITE[c.sameSite]) || {}),
  };
}

module.exports = { LoginState, cdpCookies };
