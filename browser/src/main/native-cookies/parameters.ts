/** Cookie inputs are fully checked before any native batch mutation; unsupported semantics are rejected. */
import type { CookiesSetDetails } from 'electron';
import { COOKIE_LIMITS, COOKIE_SAME_SITE } from './constants.ts';
/** URL context is mandatory; no domain-derived URL or active-tab guessing. */
export function cookieURL(value: unknown): string {
  if (typeof value !== 'string') throw Error('An explicit cookie URL is required');
  const url = new URL(value);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password)
    throw Error('Unsupported cookie URL');
  return url.href;
}
/** Names and values never coerce arbitrary objects and cannot contain cookie separators or controls. */
export function cookieString(value: unknown, name = false): string {
  if (typeof value !== 'string' || /[\p{Cc}\s;]/u.test(value) || (name && /[=,]/.test(value)))
    throw Error('Invalid cookie name or value');
  if (Buffer.byteLength(value) > COOKIE_LIMITS.bytes) throw Error('Native cookie byte limit exceeded');
  return value;
}
/** Validate all entries up front, preserving native write failures rather than reporting false atomic success. */
export function cookieBatch(value: unknown): CookiesSetDetails[] {
  if (!Array.isArray(value) || value.length > COOKIE_LIMITS.batch) throw Error('Invalid native cookie batch');
  return value.map(cookieParameters);
}
/** Only implemented attributes can cross into the native cookie manager. */
function cookieParameters(value: unknown): CookiesSetDetails {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('Invalid cookie');
  const p = value as Record<string, unknown>;
  const keys = ['name', 'value', 'url', 'domain', 'path', 'secure', 'httpOnly', 'sameSite', 'expires'];
  if (Object.keys(p).some((key) => !keys.includes(key))) throw Error('Unsupported native cookie attribute');
  const name = cookieString(p.name, true),
    text = cookieString(p.value);
  if (Buffer.byteLength(name + text) > COOKIE_LIMITS.bytes) throw Error('Native cookie byte limit exceeded');
  return { url: cookieURL(p.url), name, value: text, ...attributes(p), ...expiration(p.expires) };
}
/** Domain/path syntax and boolean attributes are explicit, while native code validates domain eligibility. */
function attributes(p: Record<string, unknown>): Partial<CookiesSetDetails> {
  return { ...flags(p), ...scope(p), ...sameSite(p.sameSite) };
}
/** Boolean flags are never coerced from strings or numbers. */
function flags(p: Record<string, unknown>): Partial<CookiesSetDetails> {
  for (const key of ['secure', 'httpOnly'])
    if (p[key] !== undefined && typeof p[key] !== 'boolean') throw Error('Cookie flags must be boolean');
  return { secure: p.secure as boolean, httpOnly: p.httpOnly as boolean };
}
/** Preserve explicit domain/path scope; the native cookie manager validates host eligibility. */
function scope(p: Record<string, unknown>): Partial<CookiesSetDetails> {
  if (p.domain !== undefined && (typeof p.domain !== 'string' || !/^\.?[a-zA-Z0-9.-]+$/.test(p.domain)))
    throw Error('Invalid cookie domain');
  if (p.path !== undefined && (typeof p.path !== 'string' || !p.path.startsWith('/') || /[\p{Cc}\s;]/u.test(p.path)))
    throw Error('Invalid cookie path');
  return { domain: p.domain as string, path: p.path as string };
}
/** An omitted SameSite value is left to the native cookie manager; it is never reported as an explicit None. */
function sameSite(value: unknown): Partial<CookiesSetDetails> {
  if (value === undefined) return {};
  if (typeof value !== 'string' || !Object.hasOwn(COOKIE_SAME_SITE, value)) throw Error('Invalid SameSite');
  return { sameSite: COOKIE_SAME_SITE[value as keyof typeof COOKIE_SAME_SITE] };
}
/** Session and persistent cookies stay distinct; zero means an expired Unix-epoch cookie. */
function expiration(value: unknown): Partial<CookiesSetDetails> {
  if (value === undefined || value === COOKIE_LIMITS.session) return {};
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) throw Error('Invalid cookie expiration');
  return { expirationDate: value };
}
