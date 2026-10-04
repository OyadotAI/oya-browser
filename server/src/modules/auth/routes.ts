/**
 * REST routes: auth.
 */

import { Router } from 'express';
import {
  userAuthMiddleware,
  registerApiKey,
  keyExpiry,
  listApiKeys,
  deleteApiKey,
  keyDigest,
  signup,
  login,
  refreshSession,
  signOutEverywhere,
  oauthUrl,
  oauthSignup,
  verifyCaptcha,
  getProfile,
  updateProfile,
} from './service.ts';
import { track } from '../telemetry/index.ts';
import { generateKey } from './keys.ts';
import { mfaRoutes } from './mfa-routes.ts';
import { issueSession, readRefreshCookie, clearSessionCookies } from './session-cookie.ts';
import { audit } from '../../platform/audit.ts';
import { control } from '../control/service.ts';
import { consume } from '../../platform/limits.ts';
import { Status } from '../../platform/http-status.ts';
import { BEARER, KEY_PREFIX_CHARS, MIN_PASSWORD_CHARS } from './constants.ts';

/** Account and API key routes, mounted by the API. */
export const router = Router({ caseSensitive: true });
mfaRoutes(router);

/**
 * An imported key becomes an administrator credential over a project holding
 * cookie jars, MFA seeds and proxy credentials, so it has to be as hard to
 * guess as one this server mints (randomBytes(24), 32 base64url chars).
 * Without this, importing "a" was a valid, publicly guessable admin key.
 */
const IMPORTABLE_KEY = /^[A-Za-z0-9_-]{32,128}$/;

/** Answers 400 with `error`. */
const badRequest = (res, error) => res.status(Status.BAD_REQUEST).json({ error });

/** Runs `work`, which answers the request; a failure answers `status` with its message after `onFail`. */
async function guarded(res, status, work: () => Promise<unknown>, onFail = () => {}) {
  try {
    await work();
  } catch (err) {
    onFail();
    res.status(status).json({ error: err.message });
  }
}

/** Why a signup's email and password are unacceptable, or null. */
function signupProblem(email, password) {
  if (!email || !password) return 'email and password required';
  if (password.length < MIN_PASSWORD_CHARS) return 'Password must be at least 8 characters';
  return null;
}

/** Answers 400 when the request's Turnstile token does not pass; true when it was answered. */
async function captchaRefused(req, res) {
  if (await verifyCaptcha(req.body?.captcha_token)) return false;
  badRequest(res, 'Captcha check failed, please try again');
  return true;
}

/** Audits a change to one of the signed-in user's keys, named by its digest. */
function auditKey(req, action: string, digest: string) {
  const meta = req.impersonatedBy ? { impersonatedBy: req.impersonatedBy } : undefined;
  audit({ action, actorKey: null, actorUser: req.user.id, targetType: 'key', targetId: digest, meta, req });
}

/** What a key list shows for a key: its digest, prefix, project and label. */
const keyInfo = (key, label) => ({
  id: keyDigest(key),
  prefix: key.slice(0, KEY_PREFIX_CHARS),
  project: control().projectIdFor(key),
  label,
});

// ─── User Auth ────────────────────────────────────────────────────────────────

/** Creates the account in the request, counts it, and answers with its new session. */
async function signUpAndIn(req, res) {
  const { email, password, display_name } = req.body;
  const made = await signup(email, password, display_name);
  track.accountSignedUp(made.user, 'email');
  issueSession(req, res, await login(email, password));
}

/**
 * POST /auth/signup, create an account (email, password of 8+ characters,
 * optional display name) and start its session. It signs in here rather than
 * leaving that to a second request, which would need a second captcha.
 */
router.post('/auth/signup', async (req, res) => {
  const { email, password } = req.body;
  const problem = signupProblem(email, password);
  if (problem) return badRequest(res, problem);
  if (await captchaRefused(req, res)) return;
  await guarded(res, Status.BAD_REQUEST, () => signUpAndIn(req, res));
});

/** The email a sign-in names, as the limit and the audit trail key it. */
const loginEmail = (req) => String(req.body.email).trim().toLowerCase();

/** Audits a password sign-in: who it was for, how it went and, once known, the account. */
function auditLogin(req, outcome: string, userId = null, reason?: string) {
  const meta = { method: 'password', ...(reason ? { reason } : {}) };
  const subject = { actorUser: userId, targetType: 'account', targetId: loginEmail(req) };
  audit({ action: 'auth.login', outcome, actorKey: null, ...subject, meta, req });
}

/** Answers 429 once this email has had its hour's sign-in attempts; true when it was answered. */
function loginLimited(req, res) {
  const limit = consume('login', loginEmail(req));
  if (limit.allowed) return false;
  auditLogin(req, 'denied', null, 'rate_limited');
  res.set('Retry-After', String(limit.retryAfter));
  res.status(Status.TOO_MANY_REQUESTS).json({ error: 'Too many sign-in attempts, try again later' });
  return true;
}

/** Signs in with the request's email and password, audits it either way, and answers with the new session. */
async function signIn(req, res) {
  const session = await login(req.body.email, req.body.password).catch((err) => {
    auditLogin(req, 'denied');
    throw err;
  });
  auditLogin(req, 'ok', session.user.id);
  issueSession(req, res, session);
}

/**
 * POST /auth/login, sign in with email and password and start a session. The
 * captcha is checked before the per-email limit, so a script that cannot pass
 * it cannot spend a person's attempts and lock them out.
 */
router.post('/auth/login', async (req, res) => {
  const { email, password } = req.body;
  if (!email || !password) return badRequest(res, 'email and password required');
  if ((await captchaRefused(req, res)) || loginLimited(req, res)) return;
  await guarded(res, Status.UNAUTHORIZED, () => signIn(req, res));
});

/**
 * GET /auth/oauth/:provider?redirect_to=…, send the browser to sign in with
 * Google or GitHub. The session comes back to `redirect_to` (the console's
 * /auth/callback), which hands its refresh token to POST /auth/refresh.
 */
router.get('/auth/oauth/:provider', async (req, res) => {
  const redirectTo = String(req.query?.redirect_to || '');
  if (!/^https?:\/\//.test(redirectTo)) return badRequest(res, 'redirect_to required');
  await guarded(res, Status.UNAVAILABLE, async () => {
    const url = await oauthUrl(req.params.provider, redirectTo);
    if (!url) return res.status(Status.NOT_FOUND).json({ error: 'Unknown sign-in provider' });
    res.redirect(url);
  });
});

/** POST /auth/refresh, trade a refresh token (body or cookie) for a new session; a rejected one clears the cookies. */
router.post('/auth/refresh', async (req, res) => {
  const presented = req.body?.refresh_token || readRefreshCookie(req);
  if (!presented) return badRequest(res, 'refresh_token required');
  const session = async () => refreshed(req, res, await refreshSession(presented));
  await guarded(res, Status.UNAUTHORIZED, session, () => clearSessionCookies(res));
});

/**
 * Starts the session, counting a Google or GitHub sign-in as a sign-up when it
 * made the account just now. The answer then says `signed_up`, so the callback
 * page can report the sign-up to the ad pixel before it moves on.
 */
function refreshed(req, res, session) {
  // Only the OAuth callback page says `oauth`; a restore, even one sending its token in the body, does not.
  const method = req.body?.oauth === true ? oauthSignup(session.user) : null;
  if (method) track.accountSignedUp(session.user, method);
  issueSession(req, res, method ? { ...session, signed_up: method } : session);
}

/**
 * The access token to sign out with: the request's bearer token, else one
 * traded for the refresh token in the body or cookie; '' when there is neither.
 */
async function accessTokenOf(req) {
  const header = req.headers.authorization;
  if (header?.startsWith(BEARER)) return header.slice(BEARER.length);
  const refresh = req.body?.refresh_token || readRefreshCookie(req);
  return refresh ? (await refreshSession(refresh)).access_token : '';
}

/** Revokes every Supabase session of the person signing out; a failure is logged, never blocks signing out. */
async function revokeSession(req) {
  try {
    const access = await accessTokenOf(req);
    if (access) await signOutEverywhere(access);
  } catch (e) {
    console.warn('[auth] sign-out could not revoke the session:', e.message);
  }
}

/**
 * POST /auth/logout. Signing out has to reach the cookie, which the page
 * cannot clear itself, and Supabase, so the tokens stop working everywhere.
 */
router.post('/auth/logout', async (req, res) => {
  await revokeSession(req);
  clearSessionCookies(res);
  res.json({ ok: true });
});

/** GET /auth/me, the signed-in user's profile. */
router.get('/auth/me', userAuthMiddleware, async (req, res) => {
  await guarded(res, Status.INTERNAL, async () => res.json(await getProfile(req.user.id)));
});

/** Rename yourself. Email is the login and role is authority; neither moves here. */
router.patch('/auth/me', userAuthMiddleware, async (req, res) => {
  const profile = await updateProfile(req.user.id, { display_name: req.body?.display_name });
  audit({ action: 'account.update', actorKey: null, targetType: 'user', targetId: req.user.id, req });
  res.json(profile);
});

// ─── API Key Management (authenticated users) ────────────────────────────────

/** GET /auth/keys, the signed-in user's API keys, by digest and prefix. */
router.get('/auth/keys', userAuthMiddleware, async (req, res) => {
  await guarded(res, Status.INTERNAL, async () => res.json(await listApiKeys(req.user.id)));
});

/**
 * POST /auth/keys, mint an API key for the signed-in user. The only time the key
 * itself is returned. Only its digest is stored, so there is no second chance to
 * read it and nothing to hand back later. Optional `expiresInDays` (1-3650)
 * sets when it stops working; the answer's `expires_at` is null for never.
 */
router.post('/auth/keys', userAuthMiddleware, async (req, res) => {
  const expiresAt = keyExpiry(req.body?.expiresInDays);
  await guarded(res, Status.INTERNAL, () => mintKey(req, res, expiresAt));
});

/** Mints and registers a key for the signed-in user, audits and counts it, and answers with it once. */
async function mintKey(req, res, expiresAt: string | null) {
  const { label } = req.body;
  const key = generateKey();
  await registerApiKey(key, req.user.id, label, expiresAt);
  const info = keyInfo(key, label || 'Default');
  auditKey(req, 'key.create', info.id);
  track.apiKeyCreated(req.user, info.project);
  res.json({ key, ...info, expires_at: expiresAt });
}

/**
 * POST /auth/keys/import, register an existing key (32-128 URL-safe characters)
 * to the signed-in user. Optional `expiresInDays` applies when the key is new to
 * this server; re-importing one already registered keeps its expiry.
 */
router.post('/auth/keys/import', userAuthMiddleware, async (req, res) => {
  const { key, label } = req.body;
  if (!key || typeof key !== 'string' || !IMPORTABLE_KEY.test(key))
    return badRequest(res, 'key must be 32-128 characters of A-Z a-z 0-9 _ -');
  const expiresAt = keyExpiry(req.body.expiresInDays);
  const claimedFromAgent = await registerApiKey(key, req.user.id, label || 'Imported', expiresAt);
  auditKey(req, 'key.import', keyDigest(key));
  if (claimedFromAgent) track.agentKeyClaimed(req.user);
  res.json({ ok: true, claimed_from_agent: Boolean(claimedFromAgent), ...keyInfo(key, label || 'Imported') });
});

/**
 * DELETE /auth/keys/:id, revoke one of the signed-in user's keys. By digest, which
 * is what GET /auth/keys returns as `id`. The server cannot look a key up by its
 * plaintext any more, and a URL is the last place to put one anyway.
 */
router.delete('/auth/keys/:id', userAuthMiddleware, async (req, res) => {
  await guarded(res, Status.INTERNAL, async () => {
    await deleteApiKey(req.params.id, req.user.id);
    auditKey(req, 'key.revoke', req.params.id);
    res.json({ ok: true });
  });
});
