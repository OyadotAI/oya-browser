/**
 * Where the API lives and how to talk to it: URL building, auth headers, the
 * account endpoints (which run on the person's session rather than a project
 * credential), and the credential this tab drives the console with.
 */

import { impersonation, storedRefreshToken } from './auth/storage';

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

/** Bearer auth plus a JSON content type, for every authenticated request; an admin's "Login as" token rides along and wins on the server. */
export function authHeaders(token: string): HeadersInit {
  const acting = impersonation();
  return {
    Authorization: `Bearer ${token}`,
    'Content-Type': 'application/json',
    // Names the client, so a start can be counted as one a person clicked for.
    'X-Oya-Client': 'console',
    ...(acting && { 'X-Impersonate-Token': acting.token }),
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
  if (!res.ok) throw new AccountError(data?.error || fallback, data?.code);
  return data;
}

/** A refused account request, with the server's machine-readable reason (`mfa_required`) when it gave one. */
export class AccountError extends Error {
  /** The server's code for the refusal, or undefined. */
  code?: string;

  /** Carries the server's message and code. */
  constructor(message: string, code?: string) {
    super(message);
    this.code = code;
  }
}

/** Signs in with email and password; the server sets the refresh cookie. */
export const login = (email: string, password: string, captchaToken?: string) =>
  account('/auth/login', 'Login failed', {
    method: 'POST',
    credentials: 'include',
    body: JSON.stringify({ email, password, captcha_token: captchaToken }),
  });

/** Creates an account. */
export const signup = (email: string, password: string, displayName?: string, captchaToken?: string) =>
  account('/auth/signup', 'Signup failed', {
    method: 'POST',
    credentials: 'include',
    body: JSON.stringify({ email, password, display_name: displayName, captcha_token: captchaToken }),
  });

/** Where "Continue with Google/GitHub" goes; the server sends the browser on to the provider. */
export const oauthStartUrl = (provider: 'google' | 'github', redirectTo: string) =>
  `${apiUrl(`/auth/oauth/${provider}`)}?redirect_to=${encodeURIComponent(redirectTo)}`;

/**
 * With no argument this refreshes from the httpOnly cookie the server set at
 * login. The explicit token is the fallback for a console served from a
 * different origin than the API, where SameSite=Lax keeps the cookie at home.
 * `oauth` says the token just came back from Google or GitHub, so the server
 * can count a new account made that way as a sign-up, once; the answer then
 * carries `signed_up` (the provider).
 */
export const refreshToken = (refreshToken?: string, oauth = false) =>
  account('/auth/refresh', 'Refresh failed', {
    method: 'POST',
    credentials: 'include',
    body: JSON.stringify(refreshToken ? { refresh_token: refreshToken, ...(oauth ? { oauth: true } : {}) } : {}),
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

/** A page the person is sent to. */
interface Redirect {
  /** Its address. */
  url: string;
}

/** The person's plan, its allowances and what they used this period; `{enabled: false}` on a self-hosted server. */
export const getBilling = (token: string) =>
  account('/billing', 'Could not load your plan', { headers: authHeaders(token) });

/** A Stripe Checkout page to subscribe to `plan` on. */
export const billingCheckout = (token: string, plan: string) =>
  account<Redirect>('/billing/checkout', 'Could not open checkout', {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify({ plan }),
  });

/** The person's issued invoices, newest first. */
export const billingInvoices = (token: string) =>
  account('/billing/invoices', 'Could not load your invoices', { headers: authHeaders(token) });

/** The person's next invoice as it stands, line by line; `{upcoming: null}` without a subscription. */
export const billingUpcoming = (token: string) =>
  account('/billing/upcoming', 'Could not load your next invoice', { headers: authHeaders(token) });

/** The Stripe page to change card or plan, or cancel. */
export const billingPortal = (token: string) =>
  account<Redirect>('/billing/portal', 'Could not open billing', {
    method: 'POST',
    headers: authHeaders(token),
  });

/** The admin overview: accounts, plans, heaviest users, installs, downloads and the fleet. Admins only. */
export const adminOverview = (token: string) =>
  account('/admin/overview', 'Could not load the overview', { headers: authHeaders(token) });

/** Every self-hosted license issued. Admins only. */
export const adminLicenses = (token: string) =>
  account('/admin/licenses', 'Could not load licenses', { headers: authHeaders(token) });

/** Issues a self-hosted license; the answer carries its key, shown once. Admins only. */
export const adminIssueLicense = (token: string, request: Record<string, unknown>) =>
  account('/admin/licenses', 'Could not issue the license', {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify(request),
  });

/** Revokes a self-hosted license. Admins only. */
export const adminRevokeLicense = (token: string, id: string) =>
  account(`/admin/licenses/${encodeURIComponent(id)}/revoke`, 'Could not revoke the license', {
    method: 'POST',
    headers: authHeaders(token),
  });

/** One person by email: plan, usage this period and key prefixes. Admins only. */
export const adminLookup = (token: string, email: string) =>
  account(`/admin/users?email=${encodeURIComponent(email)}`, 'No account with that email', {
    headers: authHeaders(token),
  });

/** A one-hour "Login as" token for one customer. Admins only, and never for another admin. */
export const adminImpersonate = (token: string, id: string) =>
  account(`/admin/users/${encodeURIComponent(id)}/impersonate`, 'Could not log in as them', {
    method: 'POST',
    headers: authHeaders(token),
  });

/** The person's API keys (metadata only); an empty answer is an empty list. */
export const listApiKeys = async (token: string) =>
  (await account('/auth/keys', 'Failed to list keys', { headers: authHeaders(token) })) ?? [];

/** Creates a project and its key; `expiresInDays` null (or left out) is a key that never expires. */
export const createApiKey = (token: string, label?: string, expiresInDays: number | null = null) =>
  account('/auth/keys', 'Could not create the project', {
    method: 'POST',
    headers: authHeaders(token),
    body: JSON.stringify({ label, ...(expiresInDays ? { expiresInDays } : {}) }),
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

/** One authenticator app on the account. */
export interface MfaFactor {
  /** Supabase's factor id. */
  id: string;
  /** Its name, as given when it was added. */
  name: string;
  /** `verified` once a first code was accepted, else `unverified`. */
  status: string;
  /** When it was added. */
  created_at: string;
}

/** The person's authenticators and how strongly this session signed in. */
export interface MfaStatus {
  /** Their authenticator apps. */
  factors: MfaFactor[];
  /** `aal2` once this session passed a second factor, else `aal1`. */
  aal: string;
}

/** An authenticator being added: its factor, QR code (an SVG data URI) and the secret to type in instead. */
export interface MfaEnrolment {
  /** The factor to verify a first code against. */
  id: string;
  /** The QR code to scan. */
  qr_code: string;
  /** The secret, for an app that cannot scan. */
  secret: string;
}

/**
 * A POST to an MFA route. Those run on the person's own Supabase session, so
 * they carry the refresh token: the cookie, or the stored one for a console
 * on another origin.
 */
const mfaPost = <T>(token: string, path: string, fallback: string, body: object = {}) =>
  account<T>(`/auth/mfa/${path}`, fallback, {
    method: 'POST',
    credentials: 'include',
    headers: authHeaders(token),
    body: JSON.stringify({ ...body, refresh_token: storedRefreshToken() }),
  });

/** The person's authenticators and this session's level. */
export const mfaStatus = (token: string) => mfaPost<MfaStatus>(token, 'status', 'Could not load your authenticators');

/** Starts adding an authenticator app. */
export const mfaEnroll = (token: string) => mfaPost<MfaEnrolment>(token, 'enroll', 'Could not start the setup');

/** What a good code answers: the new session, with its refresh token only when the server could not set the cookie. */
export interface StepUpAnswer {
  /** The refresh token, for a console on another origin. */
  refresh_token?: string;
}

/** Checks a code; the answer is a new, two-factor session (the server sets its refresh cookie). */
export const mfaVerify = (token: string, factorId: string, code: string) =>
  mfaPost<StepUpAnswer>(token, 'verify', 'That code did not work', { factor_id: factorId, code });

/** Removes an authenticator app. */
export const mfaUnenroll = (token: string, factorId: string) =>
  mfaPost(token, 'unenroll', 'Could not remove it', { factor_id: factorId });
