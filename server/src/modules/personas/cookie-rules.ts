/**
 * What the login store accepts: which cookies are valid, when one has expired,
 * which hosts a cookie is sent to, and which localStorage is kept. Pure rules;
 * cookies.ts holds the state.
 */
import { MAX_COOKIE_HOSTS, MAX_ORIGIN_STORAGE_CHARS, MAX_STORAGE_KEY_CHARS, MS_PER_SECOND } from './constants.ts';

/** A cookie's identity in its jar: domain, path and name. */
export const cookieKey = (c) => `${c.domain}|${c.path || '/'}|${c.name}`;

/** A cookie with a name, a value and a domain; anything else is dropped. */
export const validCookie = (c) =>
  c && typeof c.name === 'string' && typeof c.value === 'string' && typeof c.domain === 'string' && c.domain.length > 0;

/** Past its expiry; session cookies (no expiry) never are. */
export const expired = (c) =>
  Number(c.expirationDate ?? c.expires) > 0 && Number(c.expirationDate ?? c.expires) <= Date.now() / MS_PER_SECOND;

/** A plain object, not null or an array. */
export const isRecord = (v) => !!v && typeof v === 'object' && !Array.isArray(v);

/** The hosts asked about, lowercased without a leading dot (at most 20). */
export const hostsOf = (domains) =>
  domains
    .filter((d) => typeof d === 'string')
    .slice(0, MAX_COOKIE_HOSTS)
    .map((d) => d.toLowerCase().replace(/^\./, ''));

/** Whether a request to any of `hosts` would carry this cookie. */
export function carries(c, hosts) {
  const domain = c.domain.toLowerCase().replace(/^\./, '');
  return hosts.some(
    (h) => h === domain || (c.hostOnly !== true && c.domain.startsWith('.') && h.endsWith('.' + domain)),
  );
}

/** An http(s) origin written exactly as its canonical form. */
export function validOrigin(origin) {
  try {
    return /^https?:$/.test(new URL(origin).protocol) && new URL(origin).origin === origin;
  } catch {
    return false;
  }
}

/** One origin's storage entries worth keeping, or null when the origin is to be skipped. */
export function storageEntries(items) {
  if (!isRecord(items)) return null;
  const entries = Object.entries(items).filter(([k, v]) => k.length <= MAX_STORAGE_KEY_CHARS && typeof v === 'string');
  return JSON.stringify(entries).length > MAX_ORIGIN_STORAGE_CHARS ? null : entries;
}
