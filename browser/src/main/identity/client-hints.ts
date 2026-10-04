/**
 * Client-hint request headers, sent the way Chrome sends them. Electron never
 * sends any (it has no client-hints delegate), so a session whose user agent
 * said Chrome went out with no `sec-ch-ua` at all: something no real Chrome
 * does, and the first thing a sign-in page or an anti-bot vendor checks.
 *
 * Chrome's rules, kept here: hints go to secure origins only; the three
 * low-entropy ones go on every request; a detailed one goes only to an origin
 * whose page asked for it with Accept-CH, and only on that origin's own pages.
 */
import type { Session, WebContents } from 'electron';
import { LEADING, NAVIGATIONS, ALWAYS, HINT_ORDER } from './constants.ts';

/** Request headers by name; a value may repeat. */
type Headers = Record<string, string | string[]>;

/** One header, name and value. */
type HeaderEntry = [string, string | string[]];

/** What the hooks read off a request. */
export interface HintRequest {
  /** Where it goes. */
  url: string;
  /** What kind of resource it fetches. */
  resourceType: string;
  /** The headers Electron would send. */
  requestHeaders: Headers;
  /** The page making it, when there is one. */
  webContents?: Pick<WebContents, 'getURL'> | null;
}

/** What the hooks read off a response. */
export interface HintResponse {
  /** Where it came from. */
  url: string;
  /** What kind of resource it was. */
  resourceType: string;
  /** Its headers, Accept-CH among them. */
  responseHeaders?: Record<string, string | string[]>;
}

/** The origin of `url`, or '' when it has none. */
function originOf(url: string): string {
  try {
    return new URL(url).origin;
  } catch {
    return '';
  }
}

/** A header's value whatever its case, as one string; '' when absent. */
function headerValue(headers: Record<string, string | string[]> | undefined, name: string): string {
  const key = Object.keys(headers || {}).find((k) => k.toLowerCase() === name);
  return key && headers ? ([] as string[]).concat(headers[key]).join(',') : '';
}

/** One session's client hints: what each origin asked for, and the headers that follow from it. */
export class ClientHints {
  /** Origin → the detailed hints its page asked for. ponytail: in memory, so a restart re-learns them on the first response; persist per partition if a site is seen to mind. */
  readonly asked = new Map<string, Set<string>>();
  /** The persona's hint values. */
  private readonly hints: Record<string, string>;

  /** `hints` is the persona's hint values by lowercase header name (identity.ts). */
  constructor(hints: Record<string, string>) {
    this.hints = hints;
  }

  /** Starts sending hints on `ses`, and learning what origins ask for. Replaces the session's previous handlers. */
  install(ses: Pick<Session, 'webRequest'>): void {
    ses.webRequest.onBeforeSendHeaders((details, callback) =>
      callback({ requestHeaders: this.headersFor(details as HintRequest) }),
    );
    ses.webRequest.onHeadersReceived((details, callback) => {
      this.learn(details);
      callback({});
    });
  }

  /** A top-level page's Accept-CH replaces what its origin asked for. */
  learn(details: HintResponse): void {
    if (details.resourceType !== 'mainFrame' || !details.url.startsWith('https:')) return;
    const asked = headerValue(details.responseHeaders, 'accept-ch').toLowerCase().split(',');
    const known = asked.map((name) => name.trim()).filter((name) => Object.hasOwn(this.hints, name));
    this.asked.set(originOf(details.url), new Set(known));
  }

  /** Whether the request goes to the origin of the page making it (or is that page). */
  firstParty(details: HintRequest): boolean {
    if (details.resourceType === 'mainFrame') return true;
    return originOf(details.webContents?.getURL?.() || '') === originOf(details.url);
  }

  /** The hint names this request carries. */
  namesFor(details: HintRequest): string[] {
    const detailed = this.firstParty(details) ? this.asked.get(originOf(details.url)) : null;
    return HINT_ORDER.filter((name) => ALWAYS.includes(name) || detailed?.has(name));
  }

  /** The request's headers with the persona's hints first and any of the engine's own removed. */
  headersFor(details: HintRequest): Headers {
    const entries = Object.entries(details.requestHeaders).filter(([name]) => !/^sec-ch-ua/i.test(name));
    if (!details.url.startsWith('https:')) return Object.fromEntries(entries);
    const hints = this.namesFor(details).map((name): HeaderEntry => [name, this.hints[name]]);
    return Object.fromEntries(inChromeOrder([...hints, ...entries], details.resourceType));
  }
}

/** `entries` with the headers Chrome leads with moved to the front in its order for this kind of request. */
function inChromeOrder(entries: HeaderEntry[], resourceType: string): HeaderEntry[] {
  const leading = LEADING[NAVIGATIONS.has(resourceType) ? 'navigation' : 'subresource'];
  // A stable sort: headers with the same rank, all the unnamed ones, keep their order.
  const ranked = entries.map((entry, i) => [entry, rankIn(leading, entry[0]), i] as const);
  return ranked.sort(([, a, i], [, b, j]) => a - b || i - j).map(([entry]) => entry);
}

/** A header's place among the leading ones; every other header ranks after them all. */
function rankIn(leading: readonly string[], name: string): number {
  const i = leading.indexOf(name.toLowerCase());
  return i === -1 ? leading.length : i;
}
