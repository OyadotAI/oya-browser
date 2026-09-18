/**
 * Who is connecting: a one-time connection ticket, or an API key in the
 * query or Authorization header. Viewers may not drive a browser.
 */
import { control } from '../control/service.ts';
import { authenticateToken } from '../auth/service.ts';
import { Status } from '../../platform/http-status.ts';
import { BEARER_PREFIX_LENGTH } from './constants.ts';

/** The caller's key and presented credential; undefined after refusing the socket. */
export async function authenticate(url: URL, req, deny) {
  try {
    const found = await credentialFor(url, req);
    if (!found) return deny(Status.UNAUTHORIZED, 'Use a connection ticket');
    if (found.principal.role === 'viewer') return deny(Status.FORBIDDEN, 'Operator permission required');
    return { token: found.principal.key, authToken: found.authToken || found.principal.key };
  } catch (e) {
    return deny(e.status === Status.UNAVAILABLE ? Status.UNAVAILABLE : Status.UNAUTHORIZED, 'Unauthorized');
  }
}

/** The principal and the token it was proven with; null when a legacy query key is disallowed. */
async function credentialFor(url: URL, req) {
  const ticket = url.searchParams.get('ticket');
  if (ticket) return redeem(ticket, url);
  if (url.searchParams.has('token') && process.env.OYA_ALLOW_LEGACY_QUERY_KEYS === 'false') return null;
  const token = presentedKey(url, req);
  return { principal: await authenticateToken(token), authToken: token };
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
