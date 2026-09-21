/**
 * REST routes: auth.
 */

import { Router } from 'express';
import {
  userAuthMiddleware,
  registerApiKey,
  listApiKeys,
  deleteApiKey,
  keyDigest,
  signup,
  login,
  refreshSession,
  getProfile,
  updateProfile,
} from './service.ts';
import { generateKey } from './keys.ts';
import { issueSession, readRefreshCookie, clearSessionCookies } from './session-cookie.ts';
import { audit } from '../../platform/audit.ts';
import { control } from '../control/service.ts';
import { Status } from '../../platform/http-status.ts';
import { KEY_PREFIX_CHARS, MIN_PASSWORD_CHARS } from './constants.ts';

/** Account and API key routes, mounted by the API. */
export const router = Router({ caseSensitive: true });

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

/** What a key list shows for a key: its digest, prefix, project and label. */
const keyInfo = (key, label) => ({
  id: keyDigest(key),
  prefix: key.slice(0, KEY_PREFIX_CHARS),
  project: control().projectIdFor(key),
  label,
});

// ─── User Auth ────────────────────────────────────────────────────────────────

/** POST /auth/signup, create an account (email, password of 8+ characters, optional display name). */
router.post('/auth/signup', async (req, res) => {
  const { email, password, display_name } = req.body;
  const problem = signupProblem(email, password);
  if (problem) return badRequest(res, problem);
  await guarded(res, Status.BAD_REQUEST, async () => res.json(await signup(email, password, display_name)));
});

/** POST /auth/login, sign in with email and password and start a session. */
router.post('/auth/login', async (req, res) => {
  const { email, password } = req.body;
  if (!email || !password) return badRequest(res, 'email and password required');
  await guarded(res, Status.UNAUTHORIZED, async () => issueSession(req, res, await login(email, password)));
});

/** POST /auth/refresh, trade a refresh token (body or cookie) for a new session; a rejected one clears the cookies. */
router.post('/auth/refresh', async (req, res) => {
  const presented = req.body?.refresh_token || readRefreshCookie(req);
  if (!presented) return badRequest(res, 'refresh_token required');
  await guarded(
    res,
    Status.UNAUTHORIZED,
    async () => issueSession(req, res, await refreshSession(presented)),
    () => clearSessionCookies(res),
  );
});

/** Signing out has to reach the cookie, which the page cannot clear itself. */
router.post('/auth/logout', (req, res) => {
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
 * read it and nothing to hand back later.
 */
router.post('/auth/keys', userAuthMiddleware, async (req, res) => {
  const { label } = req.body;
  await guarded(res, Status.INTERNAL, async () => {
    const key = generateKey();
    await registerApiKey(key, req.user.id, label);
    res.json({ key, ...keyInfo(key, label || 'Default') });
  });
});

/** POST /auth/keys/import, register an existing key (32-128 URL-safe characters) to the signed-in user. */
router.post('/auth/keys/import', userAuthMiddleware, async (req, res) => {
  const { key, label } = req.body;
  if (!key || typeof key !== 'string' || !IMPORTABLE_KEY.test(key))
    return badRequest(res, 'key must be 32-128 characters of A-Z a-z 0-9 _ -');
  await registerApiKey(key, req.user.id, label || 'Imported');
  res.json({ ok: true, ...keyInfo(key, label || 'Imported') });
});

/**
 * DELETE /auth/keys/:id, revoke one of the signed-in user's keys. By digest, which
 * is what GET /auth/keys returns as `id`. The server cannot look a key up by its
 * plaintext any more, and a URL is the last place to put one anyway.
 */
router.delete('/auth/keys/:id', userAuthMiddleware, async (req, res) => {
  await guarded(res, Status.INTERNAL, async () => {
    await deleteApiKey(req.params.id, req.user.id);
    res.json({ ok: true });
  });
});
