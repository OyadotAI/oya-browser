/**
 * Desktop pairing codes.
 *
 * The dashboard hands the desktop browser an `oya://` link so a customer can
 * sign in once and let their remote browsers inherit those cookies. The API key
 * itself must not travel in that link: a protocol URL is reachable by any web
 * page the user visits, and it lands in OS logs and shell history on the way.
 *
 * So the link carries a single-use code instead. The desktop app exchanges it
 * over HTTPS for the key. A stolen link is useless once claimed or once the
 * window closes, and the app still asks the person before acting on it — the
 * code proves the dashboard issued the link, not that the user meant to click it.
 */

import { randomBytes, createHash, timingSafeEqual } from 'crypto';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { TTL_MS, MAX_OUTSTANDING, CODE_BYTES, MIN_CODE_LENGTH, MAX_CODE_LENGTH } from './constants.ts';

/** sha256(code) -> { apiKey, expiresAt }. Hashed, so a heap dump is not a key store. */
const codes = new Map();

const digest = (code) => createHash('sha256').update(code).digest('hex');

/** Drop expired codes. */
function sweep() {
  const now = Date.now();
  for (const [hash, entry] of codes) if (entry.expiresAt <= now) codes.delete(hash);
}

/** @returns {{ code: string, expiresAt: number }} */
export function issue(apiKey, persona = 'default') {
  sweep();
  if (codes.size >= MAX_OUTSTANDING) {
    throw new HttpError(Status.TOO_MANY_REQUESTS, 'Too many outstanding pairing codes; try again shortly');
  }
  const code = randomBytes(CODE_BYTES).toString('base64url');
  const expiresAt = Date.now() + TTL_MS;
  codes.set(digest(code), { apiKey, persona, expiresAt });
  return { code, expiresAt };
}

/** Whether a claimed value is shaped like a code at all. */
const plausible = (code) =>
  typeof code === 'string' && code.length >= MIN_CODE_LENGTH && code.length <= MAX_CODE_LENGTH;

/** Removes the entry for a hash and returns it if it had not expired. */
function take(hash) {
  const entry = codes.get(hash);
  if (!entry) return null;
  codes.delete(hash);
  return entry.expiresAt <= Date.now() ? null : entry;
}

/**
 * Redeem a code. Single use: claimed or expired, it is gone.
 * @returns {string|null} the API key, or null
 */
export function claimDetails(code) {
  sweep();
  if (!plausible(code)) return null;
  const hash = digest(code);
  const entry = take(hash);
  if (!entry) return null;
  // The lookup above is a hash-table hit rather than a comparison, so there is
  // no secret-dependent branch to time. This keeps that true if it ever changes.
  const a = Buffer.from(hash),
    b = Buffer.from(digest(code));
  return a.length === b.length && timingSafeEqual(a, b) ? { apiKey: entry.apiKey, persona: entry.persona } : null;
}

/** Redeem a code for just its API key, or null. */
export function claim(code) {
  return claimDetails(code)?.apiKey || null;
}

/** Test hook. */
export function reset() {
  codes.clear();
}
/** How many unexpired codes are waiting to be claimed. */
export const outstanding = () => {
  sweep();
  return codes.size;
};
