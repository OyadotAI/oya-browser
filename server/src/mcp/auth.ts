/**
 * Who is calling an MCP endpoint. Viewers may watch but not drive, so only an
 * operator key gets through; a share link reaches only its own browser's MCP,
 * never the pool.
 */
import { authenticateToken, shareReaches } from '../modules/auth/service.ts';
import { Status } from '../platform/http-status.ts';
import { BEARER } from './constants.ts';

/** The bearer token from an Authorization header, or ''. */
const bearer = (header?: string) => (header?.startsWith(BEARER) ? header.slice(BEARER.length) : '');

/** The caller's key as `{ key }`; null once a 401, 403 or 503 has been answered. */
export async function authorize(req, res) {
  try {
    return await operatorKey(req, res);
  } catch (e) {
    const status = e.status === Status.UNAVAILABLE ? Status.UNAVAILABLE : Status.UNAUTHORIZED;
    res.status(status).json({ error: 'Missing or invalid API key' });
    return null;
  }
}

/** The key, unless it belongs to a viewer or a share reaching past its browser (answered 403). */
async function operatorKey(req, res) {
  const principal = await authenticateToken(bearer(req.headers.authorization));
  const refusal = refusalOf(principal, req.params?.browserId);
  if (!refusal) return { key: principal.key };
  res.status(Status.FORBIDDEN).json({ error: refusal });
  return null;
}

/** Why this principal may not use this MCP endpoint (the pool when browserId is unset), or null. */
function refusalOf(principal, browserId?: string) {
  if (principal.role === 'viewer') return 'Operator permission required';
  return shareReaches(principal, browserId) ? null : 'This link only grants access to its shared browser';
}
