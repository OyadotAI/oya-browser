/**
 * The session cookies the auth routes set and read.
 *
 * The refresh token is the long-lived half of a session, so it travels in an
 * httpOnly cookie that page JavaScript cannot read. In localStorage it handed
 * any XSS in the console a way to hold the session open forever.
 *
 * SameSite=Lax means the cookie only comes back to its own origin, so a console
 * served from a different origin than the API — NEXT_PUBLIC_API_URL pointed
 * elsewhere during development — would never send it. In that case the token is
 * returned in the body as before and the caller stores it; `refresh_in_cookie`
 * tells the client which of the two happened.
 */

import { SESSION_MAX_AGE_MS } from './constants.ts';

/** The httpOnly refresh-token cookie. */
const REFRESH_COOKIE = 'oya_rt';

/** Where the refresh cookie is sent: the auth routes only. */
const REFRESH_COOKIE_PATH = '/api/auth';

/**
 * A readable companion to the httpOnly cookie above, holding nothing but the
 * fact that a session exists. Page JavaScript cannot see `oya_rt`, so without
 * this the console has to POST /auth/refresh on every anonymous page load just
 * to find out there is nothing to refresh.
 */
const SESSION_HINT = 'oya_session';

/** Whether the request comes from the API's own origin, so the cookie would come back. */
function sameOrigin(req) {
  const origin = req.headers.origin;
  if (!origin) return true; // same-origin fetch, or a non-browser client
  try {
    return new URL(origin).host === req.headers.host;
  } catch {
    return false;
  }
}

/** The raw `oya_rt=…` cookie pair, if the request carries one. */
const refreshCookiePair = (req) =>
  (req.headers.cookie || '')
    .split(';')
    .map((c) => c.trim())
    .find((c) => c.startsWith(`${REFRESH_COOKIE}=`));

/** The refresh token from the `oya_rt` cookie, or '' when there is none. */
export function readRefreshCookie(req) {
  const raw = refreshCookiePair(req);
  if (!raw) return '';
  try {
    return decodeURIComponent(raw.slice(REFRESH_COOKIE.length + 1));
  } catch {
    return '';
  }
}

/** Send a new session, moving the refresh token into the httpOnly cookie when the caller is same-origin. */
export function issueSession(req, res, result) {
  const { refresh_token: refresh, ...rest } = result;
  if (!refresh || !sameOrigin(req)) return res.json(result);
  const secure = req.secure || String(req.headers['x-forwarded-proto'] || '').includes('https');
  const maxAge = SESSION_MAX_AGE_MS;
  res.cookie(REFRESH_COOKIE, refresh, { httpOnly: true, sameSite: 'lax', secure, path: REFRESH_COOKIE_PATH, maxAge });
  res.cookie(SESSION_HINT, '1', { httpOnly: false, sameSite: 'lax', secure, path: '/', maxAge });
  res.json({ ...rest, refresh_in_cookie: true });
}

/** Drop both the refresh cookie and its readable hint. */
export function clearSessionCookies(res) {
  res.clearCookie(REFRESH_COOKIE, { path: REFRESH_COOKIE_PATH });
  res.clearCookie(SESSION_HINT, { path: '/' });
}
