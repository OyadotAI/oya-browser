/**
 * Request authentication: Supabase JWTs for account routes, and API keys or
 * scoped credentials for browser, MCP and API routes, with the role limits
 * each credential carries.
 */

import { control, projectId } from '../control/service.ts';
import { db as supabase, dbAuth as supabaseAuth } from '../../platform/db.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { isEnvKey, isFleetToken, keyCache, keyDigest } from './keys.ts';
import { BEARER } from './constants.ts';

/** Methods that only read. */
const READ_METHODS = ['GET', 'HEAD'];
/** Settings paths only an administrator may change. */
const ADMIN_PATHS = /^\/(config|personas|proxies|gateway\/(providers|strategy|profiles))/i;
/**
 * Secrets and recordings are not part of the sanitized viewer surface.
 * Both guards are case-insensitive to match however the router is configured.
 */
const VIEWER_HIDDEN = /^\/(config|pool\/cookies|live|gateway\/(profiles|recordings))/i;

/** Whether the request only reads. */
const isRead = (req) => READ_METHODS.includes(req.method);

/** A role limit: when it applies, the request is refused with its error. */
type RoleRule = {
  /** Whether this principal may not make this request. */
  applies: (principal: any, req: any) => boolean;
  /** The 403 message. */
  error: string;
};

/** Role limits for project credentials, checked in order. */
const ROLE_RULES: RoleRule[] = [
  {
    applies: (principal, req) => principal.role === 'viewer' && !isRead(req),
    error: 'Viewer credentials cannot change resources',
  },
  {
    applies: (principal, req) => principal.role !== 'administrator' && ADMIN_PATHS.test(req.path) && !isRead(req),
    error: 'Administrator permission required',
  },
  {
    applies: (principal, req) => principal.role === 'viewer' && VIEWER_HIDDEN.test(req.path),
    error: 'Operator permission required',
  },
];

// ── JWT middleware (for authenticated user routes) ──

/** Requires a Supabase access token in the Authorization header and sets req.user. */
export async function userAuthMiddleware(req, res, next) {
  const header = req.headers.authorization;
  if (!header || !header.startsWith(BEARER)) return res.status(Status.UNAUTHORIZED).json({ error: 'Missing token' });
  const token = header.slice(BEARER.length);
  if (!supabaseAuth) return res.status(Status.UNAVAILABLE).json({ error: 'Auth not configured' });
  return withUser(req, res, next, token);
}

/** Sets req.user from the access token and passes the request on, or answers 401. */
async function withUser(req, res, next, token) {
  try {
    const { data, error } = await supabaseAuth.auth.getUser(token);
    if (error) throw error;
    req.user = data.user;
    next();
  } catch {
    return res.status(Status.UNAUTHORIZED).json({ error: 'Invalid or expired token' });
  }
}

// ── API key middleware (for browser/MCP connections) ──

/** Resolve scoped credentials without changing legacy resource ownership. A managed browser's credential works only where allowBrowser is set. */
export async function authenticateToken(token, { allowBrowser = false } = {}) {
  if (!token) throw new HttpError(Status.UNAUTHORIZED, 'Missing API key');
  const scoped = token.startsWith('oya_') ? await scopedPrincipal(token, allowBrowser) : null;
  if (scoped) return scoped;
  const project = await control().store.get('project', projectId(token));
  if (project?.deletedAt) throw new HttpError(Status.GONE, 'Project has been deleted');
  if (isEnvKey(token) || isFleetToken(token)) return { key: token, role: 'administrator' };
  if (await isStoredKey(keyDigest(token))) return { key: token, role: 'administrator' };
  throw new HttpError(Status.FORBIDDEN, 'Invalid API key');
}

/** The principal behind an `oya_` credential, or null when the control plane does not know it. */
async function scopedPrincipal(token, allowBrowser) {
  const principal = await control().authenticate(token);
  if (principal?.role === 'browser' && !allowBrowser)
    throw new HttpError(Status.FORBIDDEN, 'Managed browser credentials cannot call this API');
  return principal || null;
}

/** Whether a digest belongs to a stored key: asked of Supabase when there is one, keeping the cache in step. */
async function isStoredKey(digest) {
  if (!supabase) return keyCache.has(digest);
  const { data, error } = await supabase.from('api_keys').select('key_hash').eq('key_hash', digest).maybeSingle();
  if (error) throw new HttpError(Status.UNAVAILABLE, 'Credential validation unavailable');
  if (data) keyCache.add(digest);
  else keyCache.delete(digest);
  return !!data;
}

/** Authenticates the bearer token for API routes and enforces role limits: share links reach only their own browser, viewers only read, and settings writes need an administrator. */
export async function authMiddleware(req, res, next) {
  try {
    await admit(req, res, next);
  } catch (err) {
    res.status(err.status || Status.UNAVAILABLE).json({ error: err.message });
  }
}

/** Authenticates, applies the credential's limits, and passes the request on under the principal's key. */
async function admit(req, res, next) {
  const token = req.authToken || bearerToken(req.headers.authorization);
  const principal = await authenticateToken(token);
  req.authToken = token;
  req.principal = principal;
  const refusal = refusalFor(principal, req);
  if (refusal) return res.status(Status.FORBIDDEN).json({ error: refusal });
  req.headers.authorization = `Bearer ${principal.key}`;
  next();
}

/** The token in a `Bearer` Authorization header, or ''. */
const bearerToken = (header) => (header?.startsWith(BEARER) ? header.slice(BEARER.length) : '');

/** Why this principal may not make this request, or null when it may. */
function refusalFor(principal, req) {
  // A share credential is scoped to one session (sessionId set, not a managed
  // browser). It may only reach its own browser's live view, stream ticket,
  // status and, for an operator share, input and control. Everything else is
  // refused, so a shareable link can never see or touch the rest of the project.
  if (principal.sessionId && principal.role !== 'browser')
    return shareAllows(principal, req) ? null : 'This link only grants access to its shared browser';
  return ROLE_RULES.find((rule) => rule.applies(principal, req))?.error ?? null;
}

/**
 * Where a share may go: read-only status/live endpoints, the POST that mints a
 * stream ticket, and, for a control share only, the POSTs that act.
 */
const sharePaths = (sid) => ({
  viewRead: [`/api/live/${sid}`, `/api/browsers/${sid}`, `/api/control/sessions/${sid}`],
  act: [`/api/control/sessions/${sid}/input`, `/api/control/sessions/${sid}/control`],
  ticket: `/api/control/sessions/${sid}/ticket`,
});

/** Whether a share credential may reach this path. */
function shareAllows(principal, req) {
  const full = decodeURIComponent(req.baseUrl + req.path);
  const { viewRead, act, ticket } = sharePaths(principal.sessionId);
  // Method is pinned so a view share can never reach a mutating verb that a
  // whitelisted path grows.
  const post = req.method === 'POST';
  if (isRead(req) && viewRead.includes(full)) return true;
  return (post && full === ticket) || (principal.role === 'operator' && post && act.includes(full));
}
