/**
 * The Cloudflare Turnstile check on sign-in and sign-up, so scripts cannot
 * mass-create accounts or guess passwords. It behaves like getoya.ai's: off
 * when no secret is configured (local dev, self-host), and open when
 * Cloudflare cannot be reached, so an outage there never locks people out.
 */

import { TURNSTILE_TIMEOUT_MS, TURNSTILE_VERIFY_URL } from './constants.ts';

/** The Turnstile secret, or '' when the captcha is off. */
const secret = () => process.env.TURNSTILE_SECRET_KEY || '';

/** Asks Cloudflare about `token`; true for a pass, and true when Cloudflare cannot answer. */
async function askCloudflare(token: string) {
  const body = new URLSearchParams({ secret: secret(), response: token });
  const signal = AbortSignal.timeout(TURNSTILE_TIMEOUT_MS);
  try {
    const res = await fetch(TURNSTILE_VERIFY_URL, { method: 'POST', body, signal });
    return Boolean((await res.json())?.success);
  } catch {
    return true;
  }
}

/** Whether a request carrying `token` may go on to sign in or sign up. */
export async function verifyCaptcha(token: unknown) {
  if (!secret()) return true;
  if (typeof token !== 'string' || !token) return false;
  return askCloudflare(token);
}
