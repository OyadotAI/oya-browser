/**
 * Reading a JWT's expiry and deciding when to renew it.
 */
import { BASE64_PAD_MOD, MIN_REFRESH_DELAY_MS, MS_PER_SECOND, REFRESH_LEAD_SECONDS } from '../constants';

/** Decodes a Base64URL JWT segment to text. */
function decodeBase64Url(segment: string): string {
  const standard = segment.replace(/-/g, '+').replace(/_/g, '/');
  const pad = (BASE64_PAD_MOD - (standard.length % BASE64_PAD_MOD)) % BASE64_PAD_MOD;
  return atob(standard + '='.repeat(pad));
}

/** The token's `exp` claim (seconds since the epoch), or null when it has none or is not a JWT. */
export function tokenExpiry(token: string): number | null {
  try {
    const payload = JSON.parse(decodeBase64Url(token.split('.')[1]));
    return payload.exp || null;
  } catch {
    return null;
  }
}

/** Milliseconds until a token expiring at `exp` should be renewed: a minute early, but never immediately. */
export function refreshDelay(exp: number, now: number): number {
  return Math.max(MIN_REFRESH_DELAY_MS, (exp - REFRESH_LEAD_SECONDS) * MS_PER_SECOND - now);
}
