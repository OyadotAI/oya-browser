/**
 * What we read out of the user's launched browser over CDP: its real version
 * (so our headers match theirs) and its cookies, already decrypted by the
 * browser itself. Kept apart from the launch mechanics in capture.ts.
 */
import { HEADLESS_TOKEN } from './constants.ts';

/** Anything that sends one CDP command and answers its result (CdpWs, or a test fake). */
export interface CdpSender {
  /** Sends `method` and resolves with its result. */
  send(method: string): Promise<unknown>;
}

/** A cookie as the pool stores it, the same for every source browser. */
export interface SlimCookie {
  /** The cookie's name. */
  name: string;
  /** Its value. */
  value: string;
  /** Its domain; a leading dot makes it a domain cookie. */
  domain: string;
  /** Its path. */
  path: string;
  /** Sent over HTTPS only. */
  secure: boolean;
  /** Hidden from page scripts. */
  httpOnly: boolean;
  /** Strict, Lax, None or unspecified. */
  sameSite: string;
  /** Sent only to the exact host. */
  hostOnly: boolean;
  /** When it expires, in seconds; absent for a session cookie. */
  expirationDate?: number;
}

/** A cookie as CDP's Storage.getCookies reports it. */
interface CdpCookie {
  /** The cookie's name. */
  name: string;
  /** Its value. */
  value: string;
  /** Its domain. */
  domain: string;
  /** Its path. */
  path?: string;
  /** Sent over HTTPS only. */
  secure?: boolean;
  /** Hidden from page scripts. */
  httpOnly?: boolean;
  /** Chromium's sameSite spelling, when set. */
  sameSite?: string;
  /** Expiry in seconds, or -1. */
  expires?: number;
  /** A session cookie, which carries no expiry. */
  session?: boolean;
}

/** What Browser.getVersion answers. */
interface CdpVersion {
  /** The browser's user agent. */
  userAgent: string;
  /** `Name/version`, such as `HeadlessChrome/128.0.6613.0`. */
  product?: string;
}

/** What Storage.getCookies answers. */
interface CdpCookies {
  /** Every cookie in the profile. */
  cookies?: CdpCookie[];
}

/** The real browser's identity, as the persona presents it. */
export interface BrowserVersion {
  /** The user agent, with any headless token removed. */
  userAgent: string;
  /** The Chrome version alone, such as `128.0.6613.0`. */
  chromeVersion: string;
}

/** Chromium's sameSite spellings; a cookie without one is unspecified. */
const SAME_SITE = new Set(['Strict', 'Lax', 'None']);

/** The real user agent and Chrome version, with any headless token removed. */
export async function readVersion(cdp: CdpSender): Promise<BrowserVersion> {
  const { userAgent, product } = (await cdp.send('Browser.getVersion')) as CdpVersion;
  return { userAgent: userAgent.replace(HEADLESS_TOKEN, 'Chrome'), chromeVersion: (product || '').split('/')[1] || '' };
}

/** Every cookie in the profile, decrypted by the browser, in the pool's slim shape. */
export async function readCookies(cdp: CdpSender): Promise<SlimCookie[]> {
  const { cookies } = (await cdp.send('Storage.getCookies')) as CdpCookies;
  return (cookies || []).map(slim);
}

/** One CDP cookie as the server stores it. */
function slim(c: CdpCookie): SlimCookie {
  return { name: c.name, value: c.value, domain: c.domain, path: c.path || '/', ...slimFlags(c) };
}

/** The cookie's flags and expiry in the server's spelling; a session cookie carries no expiry. */
function slimFlags(c: CdpCookie): Omit<SlimCookie, 'name' | 'value' | 'domain' | 'path'> {
  return {
    secure: !!c.secure,
    httpOnly: !!c.httpOnly,
    sameSite: c.sameSite && SAME_SITE.has(c.sameSite) ? c.sameSite : 'unspecified',
    hostOnly: !c.domain.startsWith('.'),
    ...(c.session || !((c.expires ?? 0) > 0) ? {} : { expirationDate: c.expires }),
  };
}
