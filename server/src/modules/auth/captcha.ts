/**
 * The Cloudflare Turnstile check on sign-in and sign-up, so scripts cannot
 * mass-create accounts or guess passwords. Off when no secret is configured
 * (local dev, self-host). Once a secret is configured it fails closed: when
 * Cloudflare cannot be reached or answers nonsense, the request is refused,
 * since an open fallback would let a script turn the check off by timing out.
 */

import { TURNSTILE_TIMEOUT_MS, TURNSTILE_VERIFY_URL } from './constants.ts';

/** The Turnstile secret, or '' when the captcha is off. */
const secret = () => process.env.TURNSTILE_SECRET_KEY || '';

/** Asks Cloudflare about `token`; true only for a pass, false when Cloudflare cannot answer. */
async function askCloudflare(token: string) {
  const body = new URLSearchParams({ secret: secret(), response: token });
  const signal = AbortSignal.timeout(TURNSTILE_TIMEOUT_MS);
  try {
    const res = await fetch(TURNSTILE_VERIFY_URL, { method: 'POST', body, signal });
    return Boolean((await res.json())?.success);
  } catch (e) {
    return failedClosed(e);
  }
}

/** Logs why Cloudflare could not answer, and refuses. */
function failedClosed(e: Error) {
  console.warn('[auth] captcha check failed closed:', e.message);
  return false;
}

/** Whether a request carrying `token` may go on to sign in or sign up. */
export async function verifyCaptcha(token: unknown) {
  if (!secret()) return true;
  if (typeof token !== 'string' || !token) return false;
  return askCloudflare(token);
}
