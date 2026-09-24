/**
 * User accounts on Supabase Auth: signing up, signing in, refreshing a
 * session, and the profile a person can see and rename.
 */

import { db as supabase, dbAuth as supabaseAuth } from '../../platform/db.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_DISPLAY_NAME, NEW_ACCOUNT_MS } from './constants.ts';
import type { SignupMethod } from '../telemetry/index.ts';

/** The profile columns a person sees. */
const PROFILE_COLUMNS = 'id, email, display_name, role, created_at';

/** Refuses when Supabase Auth is not configured. */
function requireAuth() {
  if (!supabaseAuth) throw new HttpError(Status.UNAVAILABLE, 'Database not configured');
}

/** The id and email of the user in a Supabase auth answer, with when the account was made and how, for counting sign-ups. */
const userOf = (data) => ({
  id: data.user.id,
  email: data.user.email,
  created_at: data.user.created_at,
  provider: data.user.app_metadata?.provider,
});

/** The user and session tokens in a Supabase auth answer. */
const sessionOf = (data) => ({
  user: userOf(data),
  access_token: data.session.access_token,
  refresh_token: data.session.refresh_token,
  expires_at: data.session.expires_at,
});

// ── Signup / Login ──

/** Creates a confirmed Supabase user, defaulting the display name to the email's local part. */
export async function signup(email, password, displayName) {
  requireAuth();
  const display_name = displayName || email.split('@')[0];
  const request = { email, password, email_confirm: true, user_metadata: { display_name } };
  const { data, error } = await supabaseAuth.auth.admin.createUser(request);
  if (error) throw error;
  return { user: userOf(data) };
}

/** Email/password sign-in; returns the user and session tokens. */
export async function login(email, password) {
  requireAuth();
  const { data, error } = await supabaseAuth.auth.signInWithPassword({ email, password });
  if (error) throw error;
  return sessionOf(data);
}

/** Exchanges a refresh token for a new session. */
export async function refreshSession(refreshToken) {
  requireAuth();
  const { data, error } = await supabaseAuth.auth.refreshSession({ refresh_token: refreshToken });
  if (error) throw error;
  return sessionOf(data);
}

/** The OAuth providers a person can sign in with, as Supabase names them. */
const OAUTH_PROVIDERS = { google: 'google', github: 'github' } as const;

/**
 * Where to send a browser to sign in with `provider`, or null for one we do
 * not offer. Supabase's implicit flow returns the session in the fragment of
 * `redirectTo`, which the project's redirect allowlist must contain.
 */
export async function oauthUrl(provider, redirectTo) {
  if (!Object.hasOwn(OAUTH_PROVIDERS, provider)) return null;
  requireAuth();
  const options = { redirectTo, skipBrowserRedirect: true };
  const { data, error } = await supabaseAuth.auth.signInWithOAuth({ provider: OAUTH_PROVIDERS[provider], options });
  if (error) throw error;
  return data.url;
}

/**
 * How a sign-in made the account, when it did: Supabase makes a Google or
 * GitHub account on its first sign-in, so one made in the last few minutes is
 * a sign-up. Null for a returning person, a password account, or another provider.
 */
export function oauthSignup(user, now = Date.now()): SignupMethod | null {
  const method = Object.hasOwn(OAUTH_PROVIDERS, user?.provider) ? (user.provider as 'google' | 'github') : null;
  return method && now - Date.parse(user.created_at) < NEW_ACCOUNT_MS ? method : null;
}

// ── User profile ──

/** A user's profile row, or null without Supabase. */
export async function getProfile(userId) {
  if (!supabase) return null;
  const { data, error } = await supabase.from('profiles').select(PROFILE_COLUMNS).eq('id', userId).single();
  if (error) throw error;
  return data;
}

/**
 * Change what a person is allowed to change about themselves: their name.
 *
 * Email is the login and role is an authority grant, so neither is editable
 * here, a profile form that could raise its own role would be a privilege
 * escalation with a text input in front of it.
 */
export async function updateProfile(userId, { display_name }) {
  if (!supabase) throw new HttpError(Status.CONFLICT, 'Accounts need Supabase');
  const name = String(display_name ?? '')
    .trim()
    .slice(0, MAX_DISPLAY_NAME);
  if (!name) throw new HttpError(Status.BAD_REQUEST, 'display_name cannot be empty');
  return saveDisplayName(userId, name);
}

/** Writes the new name and returns the updated profile. */
async function saveDisplayName(userId, name) {
  const { data, error } = await supabase
    .from('profiles')
    .update({ display_name: name })
    .eq('id', userId)
    .select(PROFILE_COLUMNS)
    .single();
  if (error) throw error;
  return data;
}
