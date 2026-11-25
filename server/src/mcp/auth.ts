/**
 * Who is calling an MCP endpoint. Viewers may watch but not drive, so only an
 * operator key gets through; a share link reaches only its own browser's MCP,
 * never the pool.
 */
import { authenticateToken, isShare, shareReaches } from '../modules/auth/service.ts';
import { Status } from '../platform/http-status.ts';
import { BEARER } from './constants.ts';
import { challenge } from './oauth/index.ts';

/** The bearer token from an Authorization header, or ''. */
const bearer = (header?: string) => (header?.startsWith(BEARER) ? header.slice(BEARER.length) : '');

/**
 * A chat app (claude.ai, ChatGPT) signed in over OAuth, as its credential is marked. It gets the
 * chat tool set and one browser of its own, keyed by `seat` (its credential), so it does not
 * share a target with an SDK or another connector in the same project.
 */
const connectorSeat = (principal): string | undefined =>
  principal.connector && principal.credentialId ? String(principal.credentialId) : undefined;

/** Who got through: their project key, whether a share link, and a chat connector's seat. */
interface Caller {
  /** The project key. */
  key: string;
  /** True for a share link, which reaches one browser only. */
  share?: boolean;
  /** A chat connector's credential id. */
  seat?: string;
}

/** The caller a principal is: a share, a connector with its seat, or a plain key. */
function callerOf(principal): Caller {
  if (isShare(principal)) return { key: principal.key, share: true };
  const seat = connectorSeat(principal);
  return seat ? { key: principal.key, seat } : { key: principal.key };
}

/** The caller's key as `{ key }` (plus `seat` for a chat connector); null once a 401, 403 or 503 has been answered. */
export async function authorize(req, res): Promise<Caller | null> {
  try {
    return await operatorKey(req, res);
  } catch (e) {
    const status = e.status === Status.UNAVAILABLE ? Status.UNAVAILABLE : Status.UNAUTHORIZED;
    // Where an OAuth client (claude.ai) starts signing in; a client sending a key ignores it.
    if (status === Status.UNAUTHORIZED && challenge()) res.set('WWW-Authenticate', challenge());
    res.status(status).json({ error: 'Missing or invalid API key' });
    return null;
  }
}

/** The key, unless it belongs to a viewer or a share reaching past its browser (answered 403). */
async function operatorKey(req, res): Promise<Caller | null> {
  const principal = await authenticateToken(bearer(req.headers.authorization));
  const refusal = refusalOf(principal, req.params?.browserId);
  if (!refusal) return callerOf(principal);
  res.status(Status.FORBIDDEN).json({ error: refusal });
  return null;
}

/** Why this principal may not use this MCP endpoint (the pool when browserId is unset), or null. */
function refusalOf(principal, browserId?: string) {
  if (principal.role === 'viewer') return 'Operator permission required';
  return shareReaches(principal, browserId) ? null : 'This link only grants access to its shared browser';
}
