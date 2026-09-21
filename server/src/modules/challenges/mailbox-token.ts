/**
 * Access tokens for the mailboxes one-time codes are read from, minted from a
 * stored refresh token and cached until shortly before they expire.
 *
 * Refresh tokens are credential material. They are sealed by the caller
 * (mfa.js), never logged, and never returned by the API.
 */

import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import {
  DEFAULT_TOKEN_LIFETIME_S,
  ERROR_DETAIL_CHARS,
  MAILBOX_REQUEST_TIMEOUT_MS,
  MS_PER_SECOND,
  TOKEN_REFRESH_MARGIN_MS,
} from './constants.ts';

/** Token endpoints: Google's, and the Microsoft host a tenant path is added to. */
const TOKEN_HOSTS = { google: 'https://oauth2.googleapis.com/token', microsoft: 'https://login.microsoftonline.com' };

/** access tokens live ~1h; a 90s poll window must not re-mint one every 5s. */
const tokens = new Map(); // refreshToken -> { value, expires }

/** A current access token for the mailbox, minted from its refresh token and cached until a minute before expiry. */
export async function accessToken(kind, config) {
  const cached = tokens.get(config.refreshToken);
  if (cached && cached.expires > Date.now() + TOKEN_REFRESH_MARGIN_MS) return cached.value;
  const json = await mintToken(kind, config);
  tokens.set(config.refreshToken, {
    value: json.access_token,
    expires: Date.now() + (Number(json.expires_in) || DEFAULT_TOKEN_LIFETIME_S) * MS_PER_SECOND,
  });
  return json.access_token;
}

/** Trades the refresh token for a fresh access token at the provider. */
async function mintToken(kind, config) {
  const res = await fetch(tokenUrl(kind, config), {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: tokenRequest(kind, config),
    signal: AbortSignal.timeout(MAILBOX_REQUEST_TIMEOUT_MS),
  });
  if (!res.ok) throw await refreshFailure(res);
  return res.json();
}

/** Google's token endpoint, or the tenant's Microsoft one. */
function tokenUrl(kind, config) {
  if (kind === 'gmail') return TOKEN_HOSTS.google;
  return `${TOKEN_HOSTS.microsoft}/${encodeURIComponent(config.tenant || 'common')}/oauth2/v2.0/token`;
}

/** The refresh-token grant, with the client secret and Graph scope where they apply. */
function tokenRequest(kind, config) {
  return new URLSearchParams({
    grant_type: 'refresh_token',
    refresh_token: config.refreshToken,
    client_id: config.clientId,
    ...(config.clientSecret ? { client_secret: config.clientSecret } : {}),
    ...(kind === 'graph' ? { scope: 'https://graph.microsoft.com/Mail.Read offline_access' } : {}),
  });
}

/**
 * The body carries the provider's reason (invalid_grant on a revoked token),
 * which is the one thing that makes this fixable without guessing.
 */
async function refreshFailure(res) {
  const detail = await res.text().catch(() => '');
  return new HttpError(
    Status.BAD_GATEWAY,
    `Mailbox token refresh failed (${res.status}) ${detail.slice(0, ERROR_DETAIL_CHARS)}`,
  );
}

/** Forgets every cached access token. */
export function clearTokens() {
  tokens.clear();
}
