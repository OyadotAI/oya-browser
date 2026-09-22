/**
 * Where the API lives and how to talk to it: URL building, auth headers, the
 * account endpoints (which run on the person's session rather than a project
 * credential), and the credential this tab drives the console with.
 */

/** The API base: NEXT_PUBLIC_API_URL when the API is on another origin, else same-origin `/api`. */
const API_URL = process.env.NEXT_PUBLIC_API_URL || '/api';
/** Base for resolving a relative API_URL during server rendering, where there is no window. */
const SERVER_RENDER_ORIGIN = 'http://localhost:3100';

/** The absolute or same-origin URL of an API path. */
export function apiUrl(path: string): string {
  return `${API_URL}${path}`;
}

/** API deployment, which can differ from the UI origin during development. */
export function apiOrigin(): string {
  const url = new URL(API_URL, typeof window === 'undefined' ? SERVER_RENDER_ORIGIN : window.location.origin);
  return url.origin;
}

/** Bearer auth plus a JSON content type, for every authenticated request. */
export function authHeaders(token: string): HeadersInit {
  return {
    Authorization: `Bearer ${token}`,
    'Content-Type': 'application/json',
    // Names the client, so a start can be counted as one a person clicked for.
    'X-Oya-Client': 'console',
  };
}

/**
 * The account endpoints. They run on the person's session (cookie or user token),
 * not a project credential, so a refusal here is not a reason to renew one, which
 * is why these do not go through api-client's api().
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function account<T = any>(path: string, fallback: string, init: RequestInit = {}): Promise<T> {
  const res = await fetch(apiUrl(path), { ...init, headers: { 'Content-Type': 'application/json', ...init.headers } });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data?.error || fallback);
  return data;
}

/** Signs in with email and password; the server sets the refresh cookie. */
export const login = (email: string, password: string) =>
  account('/auth/login', 'Login failed', {
    method: 'POST',
    credentials: 'include',
    body: JSON.stringify({ email, password }),
  });

/** Creates an account. */
export const signup = (email: string, password: string, displayName?: string) =>
  account('/auth/signup', 'Signup failed', {
    method: 'POST',
    body: JSON.stringify({ email, password, display_name: displayName }),
  });

/**
 * With no argument this refreshes from the httpOnly cookie the server set at
 * login. The explicit token is the fallback for a console served from a
 * different origin than the API, where SameSite=Lax keeps the cookie at home.
 */
export const refreshToken = (refreshToken?: string) =>
  account('/auth/refresh', 'Refresh failed', {
    method: 'POST',
    credentials: 'include',
    body: JSON.stringify(refreshToken ? { refresh_token: refreshToken } : {}),
  });

/** Ends the session server-side; the refresh cookie is httpOnly, so only the server can clear it. */
export async function logout() {
  await fetch(apiUrl('/auth/logout'), { method: 'POST', credentials: 'include' }).catch(() => {});
}

/** Saves the person's display name. */
export const updateProfile = (token: string, displayName: string) =>
  account('/auth/me', 'Could not save your profile', {
    method: 'PATCH',
    headers: authHeaders(token),
    body: JSON.stringify({ display_name: displayName }),
  });

/** The signed-in person's profile; anything but an object is an error. */
export async function getProfile(token: string) {
  const data = await account('/auth/me', 'Failed to fetch profile', { headers: authHeaders(token) });
  if (!data || typeof data !== 'object') throw new Error('Invalid profile response');
  return data;
}

/** The person's API keys (metadata only); an empty answer is an empty list. */
export const listApiKeys = async (token: string) =>
  (await account('/auth/keys', 'Failed to list keys', { headers: authHeaders(token) })) ?? [];

/** Creates a project and its key. */
export const createApiKey = (token: string, label?: string) =>
  account('/auth/keys', 'Could not create the project', {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify({ label }),
  });

/** Adds an existing key to the person's account. */
export const importApiKey = (token: string, key: string, label?: string) =>
  account('/auth/keys/import', 'Failed to import key', {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify({ key, label }),
  });

/**
 * Deletes a key. A key is arbitrary user input (importApiKey takes whatever is
 * pasted), so an unencoded one containing ../ sends this DELETE, with the
 * user's own bearer token, to a path they did not choose.
 */
export const deleteApiKey = (token: string, key: string) =>
  account(`/auth/keys/${encodeURIComponent(key)}`, 'Failed to delete key', {
    method: 'DELETE',
    headers: authHeaders(token),
  });

/**
 * The credential the console drives the API with.
 *
 * sessionStorage, never localStorage: this is either a one-hour project
 * credential or, for the key-only sign-in, where there is no account to mint
 * one against, the API key itself. Either way it is an administrator
 * credential for a browser fleet, and a tab is as long as it should outlive
 * the person looking at it.
 */
export const CONSOLE_KEY = 'oya_console_key';

/** This tab's project credential, else its console key, else empty (also when storage is blocked). */
export function consoleCredential(): string {
  try {
    return sessionStorage.getItem('oya_project_credential') || sessionStorage.getItem(CONSOLE_KEY) || '';
  } catch {
    return '';
  }
}
