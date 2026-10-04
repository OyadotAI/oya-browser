/**
 * Shared localStorage transport for desktop and CDP browsers. No page globals.
 * The server imports this file too (drivers/cdp), so, like the rest of
 * src/page/, it imports nothing of its own.
 */

/** Saved localStorage: origin to its key/value pairs. */
export type Origins = Record<string, Record<string, string>>;

/** Sends one CDP command on the page's session and resolves with its result. */
export type CdpSend = (method: string, params?: object) => Promise<unknown>;

/** Subscribes to one CDP event on the page's session. */
export type CdpOn = (event: string, listener: (params: unknown) => void) => unknown;

/** One attached page: its CDP seam, its current restore script, and the queue its refreshes run in. */
interface Page {
  /** The page's CDP `send`. */
  send: CdpSend;
  /** The installed restore script's identifier, once there is one. */
  script: string | null;
  /** Refreshes run one after another. */
  queue: Promise<void>;
  /** Re-installs this page's restore script. */
  refresh: () => Promise<void>;
}

/** A CDP frame, as Page.frameNavigated describes it. */
interface Frame {
  /** The parent frame, absent for a top-level frame. */
  parentId?: string;
  /** The frame's address. */
  url: string;
}

/** The storage a DOMStorage event is about. */
interface StorageId {
  /** Whether it is localStorage (not sessionStorage). */
  isLocalStorage?: boolean;
  /** The storage's origin. */
  securityOrigin?: string;
}

/** Page.frameNavigated's parameters. */
interface NavigatedEvent {
  /** The frame that navigated. */
  frame: Frame;
}

/** A DOMStorage event's parameters. */
interface StorageEvent {
  /** The storage that changed. */
  storageId?: StorageId;
}

/** Page.addScriptToEvaluateOnNewDocument's result. */
interface ScriptAdded {
  /** The script's identifier, for removing it later. */
  identifier: string;
}

/** DOMStorage.getDOMStorageItems's result. */
interface StorageItems {
  /** Key/value pairs. */
  entries: [string, string][];
}

/** A cookie as Electron or the server spells it. */
export interface StoredCookie {
  /** The cookie's name. */
  name: string;
  /** Its value. */
  value: string;
  /** The domain, with a leading dot for a domain cookie. */
  domain: string;
  /** Its path; '/' when absent. */
  path?: string;
  /** Sent over https only. */
  secure?: boolean;
  /** Hidden from page scripts. */
  httpOnly?: boolean;
  /** Set for the exact host only (Electron's flag). */
  hostOnly?: boolean;
  /** Electron's expiry, in seconds. */
  expirationDate?: number;
  /** The server's expiry, in seconds. */
  expires?: number;
  /** Electron's or Chromium's sameSite spelling. */
  sameSite?: string;
}

/** The DOMStorage events that mean an origin's localStorage changed. */
const STORAGE_EVENTS = [
  'domStorageItemAdded',
  'domStorageItemUpdated',
  'domStorageItemRemoved',
  'domStorageItemsCleared',
];

/** Chromium's sameSite spelling, from Electron's or the server's. */
const CDP_SAME_SITE: Record<string, string> = {
  strict: 'Strict',
  lax: 'Lax',
  no_restriction: 'None',
  Strict: 'Strict',
  Lax: 'Lax',
  None: 'None',
};

/** A web origin: the only kind whose storage is carried. */
const WEB_ORIGIN = /^https?:/;

/**
 * One persona's localStorage, carried between browsers: saved values are
 * written into each origin before its scripts run, and changes are captured
 * and reported through `onChange`.
 */
export class LoginState {
  /** The saved localStorage, updated as changes are captured. */
  readonly origins: Origins;
  /** Hears each captured change: the one origin and its values. */
  private readonly onChange: (changed: Origins) => void;
  /** Origins a top-level page has loaded, which no page restores again. */
  private readonly visited = new Set<string>();
  /** The pages attached, whose restore scripts follow `visited`. */
  private readonly pages = new Set<Page>();

  /** `origins` maps an origin to its saved localStorage; `onChange` hears each captured change. */
  constructor(origins: Origins = {}, onChange: (changed: Origins) => void = () => {}) {
    this.origins = origins;
    this.onChange = onChange;
  }

  /** Starts carrying storage for one page, driven by its CDP `send` and event subscription `on`. */
  async attach(send: CdpSend, on: CdpOn): Promise<void> {
    const page: Page = { send, script: null, queue: Promise.resolve(), refresh: () => this.refresh(page) };
    this.pages.add(page);
    this.listen(send, on);
    await send('Page.enable');
    await send('DOMStorage.enable');
    await page.refresh();
  }

  /** Follows the page's navigations and localStorage changes. */
  private listen(send: CdpSend, on: CdpOn): void {
    on('Page.frameNavigated', (params) => this.navigated((params as NavigatedEvent).frame));
    const capture = (params: unknown) => this.capture(send, (params as StorageEvent).storageId);
    for (const event of STORAGE_EVENTS) on(`DOMStorage.${event}`, capture);
  }

  /** Re-installs the page's restore script with the origins not yet visited. */
  private refresh(page: Page): Promise<void> {
    page.queue = page.queue
      .then(() => this.installRestore(page))
      .catch(() => {
        this.pages.delete(page);
      });
    return page.queue;
  }

  /** Swaps the page's restore script for a current one. */
  private async installRestore(page: Page): Promise<void> {
    if (page.script) await page.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: page.script });
    const pending = Object.fromEntries(Object.entries(this.origins).filter(([origin]) => !this.visited.has(origin)));
    // Into an origin with no localStorage only. A browser that already holds values for the
    // origin holds its own, and they may be newer (a token refreshed while the socket was down):
    // writing the saved copy over them on the first visit of every run logged people out.
    // ponytail: so a token another browser refreshed never reaches one that has its own; stamp origins like cookies if a site needs that.
    const source = `(() => { try { const entries = ${JSON.stringify(pending)}[location.origin]; if (entries && !localStorage.length) for (const [key, value] of Object.entries(entries)) localStorage.setItem(key, value); } catch {} })()`;
    const result = (await page.send('Page.addScriptToEvaluateOnNewDocument', { source })) as ScriptAdded;
    page.script = result.identifier;
  }

  /** A top-level navigation: that origin has had its restore, so no page restores it again. */
  private navigated(frame: Frame): void {
    if (frame.parentId) return;
    const origin = URL.parse(frame.url)?.origin;
    if (!origin || !WEB_ORIGIN.test(origin)) return;
    this.visited.add(origin);
    for (const other of this.pages) void other.refresh();
  }

  /** Reads an origin's localStorage after it changed and reports it. */
  private async capture(send: CdpSend, storageId: StorageId | undefined): Promise<void> {
    const origin = storageId?.securityOrigin || '';
    if (!storageId?.isLocalStorage || !WEB_ORIGIN.test(origin)) return;
    const values = await readStorage(send, storageId);
    if (!values) return;
    this.origins[origin] = values;
    this.onChange({ [origin]: values });
  }
}

/** An origin's localStorage as it is now, or null when the tab or document no longer has it. */
async function readStorage(send: CdpSend, storageId: StorageId): Promise<Record<string, string> | null> {
  try {
    const { entries } = (await send('DOMStorage.getDOMStorageItems', { storageId })) as StorageItems;
    return Object.fromEntries(entries);
  } catch {
    return null;
  }
}

/** Where a cookie is set: a URL for a host-only cookie, the domain otherwise. */
function cdpCookieTarget(c: StoredCookie) {
  return c.hostOnly || !c.domain.startsWith('.')
    ? { url: `http${c.secure ? 's' : ''}://${c.domain.replace(/^\./, '')}${c.path || '/'}` }
    : { domain: c.domain };
}

/** Drop Chromium's read-only cookie fields and translate Electron's spelling. */
export function cdpCookies(cookies: StoredCookie[]): object[] {
  return cookies.map((c) => ({ ...cdpCookieBase(c), ...cdpCookieTarget(c), ...cdpCookieExtras(c) }));
}

/** Name, value, path and the two flags every cookie has. */
function cdpCookieBase(c: StoredCookie) {
  return { name: c.name, value: c.value, path: c.path || '/', secure: !!c.secure, httpOnly: !!c.httpOnly };
}

/** The expiry, when there is one, and the sameSite Chromium understands. */
function cdpCookieExtras(c: StoredCookie) {
  return {
    ...(Number(c.expirationDate ?? c.expires) > 0 ? { expires: Number(c.expirationDate ?? c.expires) } : {}),
    ...(Object.hasOwn(CDP_SAME_SITE, c.sameSite ?? '') ? { sameSite: CDP_SAME_SITE[c.sameSite ?? ''] } : {}),
  };
}
