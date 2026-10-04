/**
 * Who is connecting: a one-time connection ticket, or an API key in the
 * Authorization header. A key in the query (?token=) lands in access logs and
 * browser history, so it is refused unless OYA_ALLOW_LEGACY_QUERY_KEYS=true.
 * Viewers may not drive a browser, and a share link reaches only its own
 * session.
 */
import { control } from '../control/service.ts';
import { authenticateToken, shareReaches } from '../auth/service.ts';
import { Status } from '../../platform/http-status.ts';
import { BEARER_PREFIX_LENGTH } from './constants.ts';

/** The caller's key and presented credential; undefined after refusing the socket. */
export async function authenticate(url: URL, req, deny) {
  try {
    const found = await credentialFor(url, req);
    if (!found) return deny(Status.UNAUTHORIZED, 'Use a connection ticket');
    return callerOf(found, url, deny);
  } catch (e) {
    return deny(e.status === Status.UNAVAILABLE ? Status.UNAVAILABLE : Status.UNAUTHORIZED, 'Unauthorized');
  }
}

/** The caller's key and presented credential, or undefined after a 403 for a principal that may not use this upgrade. */
function callerOf(found, url: URL, deny) {
  const refusal = refusalOf(found.principal, url);
  if (refusal) return deny(Status.FORBIDDEN, refusal);
  return { token: found.principal.key, authToken: found.authToken || found.principal.key };
}

/** Why this principal may not use this upgrade: a viewer, or a share link reaching past its session; null when it may. */
function refusalOf(principal, url: URL) {
  if (principal.role === 'viewer') return 'Operator permission required';
  return shareReaches(principal, shareTarget(url)) ? null : 'Forbidden';
}

/**
 * The one session this upgrade names, for the share check: ?session= or
 * ?browser=. Null when it names none, names a profile, or names two that differ.
 */
function shareTarget(url: URL) {
  const session = url.searchParams.get('session');
  const browser = url.searchParams.get('browser');
  if (url.searchParams.has('profile') || (session && browser && session !== browser)) return null;
  return session || browser;
}

/** The principal and the token it was proven with; null when a query key arrives without the legacy opt-in. */
async function credentialFor(url: URL, req) {
  const ticket = url.searchParams.get('ticket');
  if (ticket) return redeem(ticket, url);
  if (url.searchParams.has('token') && !legacyQueryKeysAllowed()) return null;
  const token = presentedKey(url, req);
  return { principal: await authenticateToken(token), authToken: token };
}

/** Whether the operator opted back in to API keys in the URL; off unless OYA_ALLOW_LEGACY_QUERY_KEYS=true. */
export function legacyQueryKeysAllowed() {
  return process.env.OYA_ALLOW_LEGACY_QUERY_KEYS === 'true';
}

/** Redeems a connection ticket for the browser or session it names. */
async function redeem(ticket, url: URL) {
  const token = await control().redeem(ticket, url.searchParams.get('browser') || url.searchParams.get('session'));
  return { principal: await authenticateToken(token), authToken: token };
}

/** An API key from the query, else from a Bearer header; '' when neither. */
function presentedKey(url: URL, req) {
  return (
    url.searchParams.get('token') ||
    (req.headers.authorization?.startsWith('Bearer ') ? req.headers.authorization.slice(BEARER_PREFIX_LENGTH) : '')
  );
}
