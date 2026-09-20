/**
 * What the session keeps in browser storage, and how it is cleared. The
 * access token is never stored; only the cross-origin refresh-token fallback
 * and the console's credentials are.
 */

/** The refresh token, stored only when the server could not use a cookie. */
const REFRESH_TOKEN = 'oya_refresh_token';

/** Every key a signed-in session may have left behind. */
const LOCAL_KEYS = [
  'oya_token',
  REFRESH_TOKEN,
  'oya_api_key', // written by versions before this one
];
/** The console's per-tab credentials. */
const SESSION_KEYS = ['oya_console_key', 'oya_project_credential', 'oya_project_id'];

/** Removes everything a session stored. */
export function clearStoredSession() {
  for (const key of LOCAL_KEYS) localStorage.removeItem(key);
  for (const key of SESSION_KEYS) sessionStorage.removeItem(key);
}

/** The stored refresh token, if the server handed one over. */
export function storedRefreshToken(): string | undefined {
  return localStorage.getItem(REFRESH_TOKEN) || undefined;
}

/** Keeps the refresh token the server returned, or drops the stored one when it returned none. */
export function keepRefreshToken(token: string | undefined) {
  if (token) localStorage.setItem(REFRESH_TOKEN, token);
  else localStorage.removeItem(REFRESH_TOKEN);
}

/**
 * `oya_session` is the readable half the server sets alongside the httpOnly
 * cookie. Without it there is nothing to refresh, and asking anyway puts a
 * failed request in the console of every anonymous page load.
 */
export function hasSessionCookie(): boolean {
  return document.cookie.split('; ').some((c) => c.startsWith('oya_session='));
}
