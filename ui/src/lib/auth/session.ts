/**
 * The session's steps as plain functions over a SessionHandle: sign in,
 * refresh, restore on load, and clear. The hooks in use-auth-session.ts wire
 * them to React.
 */
import { login as apiLogin, signup as apiSignup, getProfile, refreshToken as apiRefreshToken } from '../api';
import { clearStoredSession, hasSessionCookie, keepRefreshToken, storedRefreshToken } from './storage';
import { refreshDelay, tokenExpiry } from './token';
import type { SessionHandle, User } from './types';

/** What the refresh endpoint answers. */
interface RefreshAnswer {
  /** The new access token. */
  access_token: string;
  /** A refresh token, only when the server could not set a cookie. */
  refresh_token?: string;
  /** The person, when the server includes them. */
  user?: User;
}

/** Forgets the session: state, stored credentials and the renewal timer. */
export function clearSession(h: SessionHandle) {
  h.version.current++;
  h.setUser(null);
  h.setToken(null);
  clearStoredSession();
  if (h.timer.current) clearTimeout(h.timer.current);
}

/**
 * Refresh from the httpOnly cookie the server set at login, nothing here
 * reads or writes a refresh token, which is the point: page JavaScript
 * cannot reach it, so an XSS cannot walk off with the session.
 *
 * The localStorage token is only the fallback for a console served from a
 * different origin than the API, where SameSite=Lax keeps the cookie at home
 * and the server returns the token in the body instead.
 */
export async function refreshSession(h: SessionHandle, clear: () => void): Promise<string | null> {
  const version = h.version.current;
  const stored = storedRefreshToken();
  if (!stored && !hasSessionCookie()) return null;
  const data: RefreshAnswer | null = await apiRefreshToken(stored).catch(() => null);
  if (version !== h.version.current) return null;
  if (!data) clear();
  return data ? adopt(h, data) : null;
}

/** Takes on a refreshed session and returns its access token. */
function adopt(h: SessionHandle, data: RefreshAnswer): string {
  h.setToken(data.access_token);
  // Only ever stored when the server could not use a cookie.
  keepRefreshToken(data.refresh_token);
  if (data.user) h.setUser(data.user);
  return data.access_token;
}

/**
 * The access token is never persisted, it is rebuilt from the refresh
 * cookie on every load, so a closed tab leaves nothing readable behind.
 * `current` says whether this restore still speaks for the session.
 */
export async function restoreSession(h: SessionHandle, refresh: () => Promise<string | null>, current: () => boolean) {
  const tok = await refresh();
  if (!tok) return;
  const profile = await getProfile(tok);
  if (current()) {
    h.setToken(tok);
    h.setUser(profile);
  }
}

/** Schedules the renewal of `token` a minute before it expires; returns the cancel. */
export function scheduleRefresh(h: SessionHandle, token: string, refresh: () => Promise<string | null>) {
  const exp = tokenExpiry(token);
  if (!exp) return;
  h.timer.current = setTimeout(() => void refresh(), refreshDelay(exp, Date.now()));
  return () => {
    if (h.timer.current) clearTimeout(h.timer.current);
  };
}

/** Restores the session once and then marks loading done; returns the cancel for an unmount. */
export function startRestore(h: SessionHandle, refresh: () => Promise<string | null>, clear: () => void) {
  let cancelled = false;
  const version = h.version.current;
  const current = () => !cancelled && version === h.version.current;
  restoreSession(h, refresh, current)
    .catch(() => current() && clear())
    .finally(() => !cancelled && h.setLoading(false));
  return () => void (cancelled = true);
}

/** Starts a new session version from a sign-in or sign-up answer. */
function begin(h: SessionHandle, data: RefreshAnswer) {
  h.version.current++;
  adopt(h, data);
}

/** Signs in and starts a new session version. */
export async function signIn(h: SessionHandle, email: string, password: string, captchaToken?: string) {
  begin(h, await apiLogin(email, password, captchaToken));
}

/** Creates the account; the server signs it in, so its session starts here too. */
export async function signUp(h: SessionHandle, account: SignupRequest) {
  begin(h, await apiSignup(account.email, account.password, account.displayName, account.captchaToken));
}

/** What a sign-up sends. */
export interface SignupRequest {
  /** Sign-in email. */
  email: string;
  /** The chosen password. */
  password: string;
  /** Name shown in the console, when given. */
  displayName?: string;
  /** The Turnstile token, when the captcha is on. */
  captchaToken?: string;
}
