/**
 * Every number and fixed string the auth module runs on, by name: key sizes,
 * input limits and session-cookie settings.
 */
/** Random bytes in a minted API key (32 base64url characters). */
export const KEY_BYTES = 24;
/** Leading characters of a key kept for display. */
export const KEY_PREFIX_CHARS = 8;
/** The Authorization scheme a token travels under. */
export const BEARER = 'Bearer ';
/** Longest project name taken from a key's label. */
export const MAX_PROJECT_NAME = 100;
/** Longest display name a person can give themselves. */
export const MAX_DISPLAY_NAME = 100;
/** Shortest password accepted at signup. */
export const MIN_PASSWORD_CHARS = 8;

/** Milliseconds in an hour. */
const HOUR_MS = 3_600_000;
/** Hours a console sign-in survives without use, unless OYA_SESSION_IDLE_HOURS says otherwise. */
const DEFAULT_SESSION_IDLE_HOURS = 24;
/** The idle hours from OYA_SESSION_IDLE_HOURS, or the default when it is unset or not a positive number. */
const sessionIdleHours = () => {
  const hours = Number(process.env.OYA_SESSION_IDLE_HOURS);
  return Number.isFinite(hours) && hours > 0 ? hours : DEFAULT_SESSION_IDLE_HOURS;
};
/**
 * How long the session cookies last. Every refresh sets them again, so this is
 * an idle timeout: a console left unused this long has to sign in again.
 */
export const SESSION_MAX_AGE_MS = sessionIdleHours() * HOUR_MS;

/** Cloudflare's endpoint that says whether a Turnstile token is genuine. */
export const TURNSTILE_VERIFY_URL = 'https://challenges.cloudflare.com/turnstile/v0/siteverify';
/** How long to wait for Cloudflare before refusing the request. */
export const TURNSTILE_TIMEOUT_MS = 10_000;

/** Leading hex zeros sha256(challenge + nonce) must have: about a million hashes, a second or two for one agent. */
export const AGENT_POW_ZEROS = 5;
/** How long an agent has to solve a challenge and sign up with it. */
export const AGENT_CHALLENGE_TTL_MS = 600_000;
/** Longest nonce accepted, so a signup cannot make the server hash megabytes. */
export const AGENT_NONCE_MAX_CHARS = 64;
/** Longest email accepted from an agent. */
export const AGENT_EMAIL_MAX_CHARS = 254;
/** How recently an account must have been made for a Google or GitHub sign-in to count as its sign-up: ten minutes. */
export const NEW_ACCOUNT_MS = 600_000;

/** The header an admin's "Login as" token travels in; it wins over Authorization. */
export const IMPERSONATE_HEADER = 'x-impersonate-token';
/** How long a "Login as" token lasts: one hour. */
export const IMPERSONATE_TTL_MS = 3_600_000;
/** Shortest signing secret accepted for "Login as" tokens. */
export const IMPERSONATE_MIN_SECRET = 32;
/** Only people signed in with a confirmed address at this domain are admins, unless OYA_ADMIN_EMAILS names them. */
export const ADMIN_DOMAIN = '@getoya.ai';
/** The Supabase assurance level a sign-in with a second factor carries. */
export const MFA_AAL = 'aal2';
/** The name an authenticator app shows above an Oya Browser code. */
export const MFA_ISSUER = 'Oya Browser';
/** A TOTP code as an authenticator app shows it: six digits. */
export const TOTP_CODE = /^\d{6}$/;

/** Milliseconds in a day, for API key lifetimes. */
export const DAY_MS = 86_400_000;
/** Shortest lifetime a new API key may be given, in days. */
export const MIN_KEY_DAYS = 1;
/** Longest lifetime a new API key may be given, in days: ten years. */
export const MAX_KEY_DAYS = 3650;
/** How often one expired key's refusals are audited: once an hour, so a retrying client does not flood the log. */
export const KEY_EXPIRED_AUDIT_MS = 3_600_000;
/** Characters of an ISO time that are its date (YYYY-MM-DD), for naming the day a key expired. */
export const ISO_DATE_CHARS = 10;
/** OYA_API_KEY_MAX_DAYS when it is unset: new keys may live forever. */
const DEFAULT_API_KEY_MAX_DAYS: number | null = null;

/**
 * The operator's cap on a new key's lifetime from OYA_API_KEY_MAX_DAYS, or the
 * default when it is unset. A value that is not a whole number of days in range
 * stops the server, so a typo never silently lifts the cap.
 */
export function apiKeyMaxDays(raw = process.env.OYA_API_KEY_MAX_DAYS): number | null {
  if (raw === undefined || raw.trim() === '') return DEFAULT_API_KEY_MAX_DAYS;
  const days = Number(raw);
  if (Number.isInteger(days) && days >= MIN_KEY_DAYS && days <= MAX_KEY_DAYS) return days;
  throw new Error(`OYA_API_KEY_MAX_DAYS must be a whole number of days from ${MIN_KEY_DAYS} to ${MAX_KEY_DAYS}`);
}
/** The longest a new API key may live, in days, read once at startup; null is no cap. */
export const API_KEY_MAX_DAYS = apiKeyMaxDays();

/** Mixed into the service key to derive the signing secret, so the two never coincide. */
export const IMPERSONATE_CONTEXT = 'oya-impersonate-token-v1';
