/**
 * Who is calling an MCP endpoint. Viewers may watch but not drive, so only an
 * operator key gets through.
 */
import { authenticateToken } from '../modules/auth/service.ts';
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

/** The key, unless it belongs to a viewer (answered 403). */
async function operatorKey(req, res) {
  const principal = await authenticateToken(bearer(req.headers.authorization));
  if (principal.role !== 'viewer') return { key: principal.key };
  res.status(Status.FORBIDDEN).json({ error: 'Operator permission required' });
  return null;
}
