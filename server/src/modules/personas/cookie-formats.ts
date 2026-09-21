/**
 * The formats a persona's cookie jar is exported in, so a login made once can
 * be used from anywhere: the jar as stored, Playwright's `addCookies` shape,
 * and the Netscape cookies.txt that curl, wget and yt-dlp read.
 */

/** Playwright's sameSite for each spelling the jar holds (Electron's, and Chromium's). */
const PLAYWRIGHT_SAME_SITE = {
  strict: 'Strict',
  lax: 'Lax',
  no_restriction: 'None',
  Strict: 'Strict',
  Lax: 'Lax',
  None: 'None',
};

/** A session cookie's expiry, as Playwright spells it. */
const SESSION_EXPIRES = -1;

/** A cookie's expiry in seconds, or 0 for a session cookie. */
const expiryOf = (c) => Math.max(0, Number(c.expirationDate ?? c.expires) || 0);

/** A stored cookie without the sync stamp, which is this server's bookkeeping. */
const stored = ({ t: _t, ...cookie }) => cookie;

/** Playwright's sameSite for a cookie, or nothing when the jar has none: Playwright then applies the browser's default. */
function playwrightSameSite(c) {
  return Object.hasOwn(PLAYWRIGHT_SAME_SITE, c.sameSite ?? '') ? { sameSite: PLAYWRIGHT_SAME_SITE[c.sameSite] } : {};
}

/** One cookie for Playwright's `context.addCookies`. */
function playwrightCookie(c) {
  const where = { name: c.name, value: c.value, domain: c.domain, path: c.path || '/' };
  const flags = { secure: !!c.secure, httpOnly: !!c.httpOnly, ...playwrightSameSite(c) };
  return { ...where, ...flags, expires: expiryOf(c) || SESSION_EXPIRES };
}

/** TRUE or FALSE, as cookies.txt spells a flag. */
const flag = (on) => (on ? 'TRUE' : 'FALSE');

/** One cookies.txt line: domain, subdomains, path, secure, expiry, name, value. An HttpOnly cookie carries curl's prefix. */
function netscapeLine(c) {
  const domain = (c.httpOnly ? '#HttpOnly_' : '') + c.domain;
  const fields = [domain, flag(c.domain.startsWith('.')), c.path || '/', flag(c.secure), Math.floor(expiryOf(c))];
  return [...fields, c.name, c.value].join('\t');
}

/** Each format's writer. */
const WRITERS = {
  json: (cookies) => cookies.map(stored),
  playwright: (cookies) => cookies.map(playwrightCookie),
  netscape: (cookies) => ['# Netscape HTTP Cookie File', ...cookies.map(netscapeLine), ''].join('\n'),
};

/** The formats a jar can be exported in. */
export const FORMATS = Object.keys(WRITERS);

/** The jar in `format`: a list for json and playwright, text for netscape; undefined for a format not known. */
export function formatJar(format, cookies) {
  return Object.hasOwn(WRITERS, format) ? WRITERS[format](cookies) : undefined;
}
