/**
 * Request authentication: Supabase JWTs for account routes, and API keys or
 * scoped credentials for browser, MCP and API routes, with the role limits
 * each credential carries.
 */

import { control, projectId } from '../control/service.ts';
import { dbAuth as supabaseAuth } from '../../platform/db.ts';
import { findKey } from './repository.ts';
import { HttpError, sendError } from '../../platform/errors.ts';
import { audit } from '../../platform/audit.ts';
import { Status } from '../../platform/http-status.ts';
import { cacheKey, isEnvKey, isFleetToken, keyDigest, noteAgentKey, uncacheKey } from './keys.ts';
import { BEARER, IMPERSONATE_HEADER, ISO_DATE_CHARS, KEY_EXPIRED_AUDIT_MS } from './constants.ts';
import { actingAs, readImpersonation, type Impersonation } from './impersonate.ts';
import { owesSecondFactor } from './mfa.ts';

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

/**
 * Requires a Supabase access token in the Authorization header and sets req.user.
 * An admin's "Login as" token, when present, wins: req.user is then the customer.
 */
export async function userAuthMiddleware(req, res, next) {
  if (req.headers[IMPERSONATE_HEADER]) return asImpersonated(req, res, next);
  const header = req.headers.authorization;
  if (!header || !header.startsWith(BEARER)) return res.status(Status.UNAUTHORIZED).json({ error: 'Missing token' });
  const token = header.slice(BEARER.length);
  if (!sessionUsers.configured()) return res.status(Status.UNAVAILABLE).json({ error: 'Auth not configured' });
  return withUser(req, res, next, token);
}

/** Who a console access token belongs to, asked of Supabase Auth; a test puts a fake here. */
export const sessionUsers = {
  /** Whether Supabase Auth is configured at all. */
  configured: () => Boolean(supabaseAuth),
  /** Supabase's answer for the token: the user with their factors, or an error. */
  getUser: (token: string): Promise<any> => supabaseAuth.auth.getUser(token),
};

/** What a session that still owes its second factor is told; the console sends the person to enter a code. */
const MFA_REQUIRED = { error: 'Enter the code from your authenticator app', code: 'mfa_required' };

/**
 * Sets req.user (and req.authAal, how strongly they signed in) from the access
 * token and passes the request on, or answers 401. A person with a verified
 * authenticator whose session has not passed it (aal1) is refused here, so a
 * password alone, stolen or not, opens nothing but the /auth/mfa routes.
 */
async function withUser(req, res, next, token) {
  const user = await verifiedUser(token);
  if (!user) return res.status(Status.UNAUTHORIZED).json({ error: 'Invalid or expired token' });
  if (owesSecondFactor(user, token)) return res.status(Status.UNAUTHORIZED).json(MFA_REQUIRED);
  Object.assign(req, { user, authAal: claimOf(token, 'aal') });
  next();
}

/** The user a Supabase access token belongs to (with their factors), or null when Supabase refuses it. */
async function verifiedUser(token) {
  const { data, error } = await sessionUsers.getUser(token).catch(() => ({ data: null, error: true }));
  return error ? null : data?.user || null;
}

/** One claim of a JWT that Supabase has already verified, or undefined when it cannot be read. */
function claimOf(token: string, name: string) {
  try {
    return JSON.parse(Buffer.from(token.split('.')[1], 'base64url').toString())[name];
  } catch {
    return undefined;
  }
}

/** Sets req.user to the customer a "Login as" token names, audited under the admin, or answers 401. */
async function asImpersonated(req, res, next) {
  try {
    const claims = readImpersonation(String(req.headers[IMPERSONATE_HEADER]));
    if (!claims) return res.status(Status.UNAUTHORIZED).json({ error: 'Invalid or expired Login as token' });
    await actAs(req, claims);
  } catch (err) {
    return sendError(res, err, req);
  }
  next();
}

/** Makes the request the customer's (re-checked every time, see actingAs), and leaves one audit row for it under the admin. */
async function actAs(req, claims: Impersonation) {
  req.user = await actingAs(claims);
  req.impersonatedBy = claims.impersonated_by;
  const meta = { method: req.method, path: req.originalUrl };
  const subject = { actorUser: claims.impersonated_by, targetType: 'user', targetId: claims.sub };
  audit({ action: 'admin.impersonate.request', ...subject, meta, req });
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

/** Whether a digest belongs to a stored key: asked of storage every time, keeping the cache in step. */
async function isStoredKey(digest) {
  const row = await findKey(digest).catch(() => {
    throw new HttpError(Status.UNAVAILABLE, 'Credential validation unavailable');
  });
  noteAgentKey(digest, Boolean(row?.agent_email && !row.user_id));
  refuseExpired(row);
  if (row) cacheKey(digest, row.expires_at);
  else uncacheKey(digest);
  return !!row;
}

/** When each expired key's refusal was last audited, by digest. */
const expiredAudited = new Map<string, number>();

/** Refuses a stored key past its expires_at with 401; a key without one never expires. */
function refuseExpired(row) {
  if (!row?.expires_at || Date.parse(row.expires_at) > Date.now()) return;
  uncacheKey(row.key_hash);
  auditExpired(row);
  const day = String(row.expires_at).slice(0, ISO_DATE_CHARS);
  throw new HttpError(Status.UNAUTHORIZED, `API key expired on ${day}; create a new key in the console`);
}

/** Audits an expired key's refusal as key.expired, at most once per key per KEY_EXPIRED_AUDIT_MS so a retrying client does not flood the log. */
function auditExpired(row) {
  const now = Date.now();
  if (now - (expiredAudited.get(row.key_hash) ?? -Infinity) < KEY_EXPIRED_AUDIT_MS) return;
  expiredAudited.set(row.key_hash, now);
  const subject = { actorUser: row.user_id ?? null, targetType: 'key', targetId: row.key_hash };
  audit({ action: 'key.expired', outcome: 'denied', actorKey: null, ...subject });
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

/**
 * Whether a principal is a share link's credential: bound to one session
 * (sessionId set) and not a managed browser's own credential.
 */
export const isShare = (principal) => !!principal?.sessionId && principal.role !== 'browser';

/**
 * The one confinement every non-REST entry point (CDP /connect, MCP, browser
 * /ws) applies to share links: a share may reach only the session it was
 * issued for. `target` is the session or browser id the caller names; null
 * means "no single session" (a new browser, a profile, the MCP pool, a /ws
 * registration), which a share may never reach. Any other credential passes;
 * its role limits are checked by the caller.
 */
export const shareReaches = (principal, target: string | null | undefined) =>
  !isShare(principal) || (!!target && target === principal.sessionId);

/** Why this principal may not make this request, or null when it may. */
function refusalFor(principal, req) {
  // A share credential is scoped to one session (sessionId set, not a managed
  // browser). It may only reach its own browser's live view, stream ticket,
  // status and, for an operator share, input and control. Everything else is
  // refused, so a shareable link can never see or touch the rest of the project.
  if (isShare(principal))
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
