/**
 * Two-factor sign-in with an authenticator app (TOTP), on Supabase Auth MFA.
 *
 * The console holds no Supabase session of its own: the server signs people
 * in and keeps the refresh token in an httpOnly cookie. So each MFA call opens
 * a short-lived Supabase Auth client signed in as the caller, from the access
 * token they sent and their refresh token, and drops it with the request.
 */

import { dbAuthClient } from '../../platform/db.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MFA_AAL, MFA_ISSUER } from './constants.ts';

/** Where the routes get a fresh Supabase Auth client; a test puts a fake here. */
export const mfaClient = { open: dbAuthClient };

/** The assurance level (`aal1` or `aal2`) a Supabase access token carries, or undefined when it cannot be read. */
export function aalOf(token: string) {
  try {
    return JSON.parse(Buffer.from(String(token).split('.')[1], 'base64url').toString()).aal;
  } catch {
    return undefined;
  }
}

/** Whether a Supabase user has a second factor that works. */
export const hasVerifiedFactor = (user) => (user?.factors || []).some((f) => f.status === 'verified');

/**
 * Whether a new session still owes its second factor: the person has one, but
 * signed in with a password or Google/GitHub alone. The console then asks for
 * a code before going on.
 */
export const owesSecondFactor = (user, accessToken) => hasVerifiedFactor(user) && aalOf(accessToken) !== MFA_AAL;

/** A Supabase answer's data, or its error as a 400 in Supabase's words. */
function unwrap({ data, error }) {
  if (error) throw new HttpError(Status.BAD_REQUEST, error.message);
  return data;
}

/** The caller's own Supabase Auth client and who they are; 401 when the tokens do not sign in. */
export async function openMfa(accessToken: string, refreshToken: string) {
  if (!accessToken || !refreshToken) throw new HttpError(Status.UNAUTHORIZED, 'Sign in again to continue');
  const auth = mfaClient.open();
  if (!auth) throw new HttpError(Status.UNAVAILABLE, 'Auth not configured');
  const { data, error } = await auth.setSession({ access_token: accessToken, refresh_token: refreshToken });
  if (error || !data?.user) throw new HttpError(Status.UNAUTHORIZED, 'Invalid or expired token');
  return { auth, user: data.user };
}

/** One authenticator as the console lists it. */
const factorInfo = (f) => ({ id: f.id, name: f.friendly_name || '', status: f.status, created_at: f.created_at });

/** The caller's authenticator apps, verified or not. */
async function totpFactors(auth) {
  return unwrap(await auth.mfa.listFactors()).all.filter((f) => f.factor_type === 'totp');
}

/** The caller's authenticators and how strongly this session signed in (`aal1` or `aal2`). */
export async function mfaStatus(auth) {
  const factors = (await totpFactors(auth)).map(factorInfo);
  const { currentLevel } = unwrap(await auth.mfa.getAuthenticatorAssuranceLevel());
  return { factors, aal: currentLevel };
}

/**
 * Starts adding an authenticator: the QR code (an SVG data URI) and the secret
 * to type in instead, plus the factor to verify a first code against. An
 * enrolment left unverified is removed first, so an abandoned attempt never
 * blocks the next one.
 */
export async function enrollTotp(auth) {
  for (const f of await totpFactors(auth))
    if (f.status !== 'verified') unwrap(await auth.mfa.unenroll({ factorId: f.id }));
  const friendlyName = `Authenticator added ${new Date().toISOString()}`;
  const data = unwrap(await auth.mfa.enroll({ factorType: 'totp', issuer: MFA_ISSUER, friendlyName }));
  return { id: data.id, qr_code: data.totp.qr_code, secret: data.totp.secret, uri: data.totp.uri };
}

/**
 * Checks a code from the authenticator. A first code activates a new one; any
 * good code turns the session into a two-factor (aal2) one, so Supabase issues
 * new tokens, returned here as a sign-in returns them.
 */
export async function verifyTotp(auth, factorId: string, code: string) {
  const data = unwrap(await auth.mfa.challengeAndVerify({ factorId, code }));
  const user = { id: data.user.id, email: data.user.email };
  return { user, access_token: data.access_token, refresh_token: data.refresh_token, expires_at: data.expires_at };
}

/** Removes an authenticator. Supabase refuses a verified one unless this session passed its second factor. */
export async function unenrollTotp(auth, factorId: string) {
  unwrap(await auth.mfa.unenroll({ factorId }));
}
