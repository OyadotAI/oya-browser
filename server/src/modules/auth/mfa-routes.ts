/**
 * REST routes: two-factor sign-in (TOTP). The console lists, adds, checks and
 * removes a person's authenticator apps here, and steps a session up to aal2,
 * which the admin pages need when OYA_ADMIN_REQUIRE_MFA=true.
 *
 * Every route is a POST taking the session's refresh token from the cookie
 * (or the body, for a console on another origin), because Supabase's MFA
 * calls run as the person, on their session, not on the server's key.
 */

import type { Router } from 'express';
import { audit } from '../../platform/audit.ts';
import { consume } from '../../platform/limits.ts';
import { sendError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { issueSession, readRefreshCookie } from './session-cookie.ts';
import { enrollTotp, mfaStatus, openMfa, unenrollTotp, verifyTotp } from './mfa.ts';
import { BEARER, IMPERSONATE_HEADER, TOTP_CODE } from './constants.ts';

/** The bearer token the request carries, or ''. */
const bearerOf = (req) => {
  const header = String(req.headers.authorization || '');
  return header.startsWith(BEARER) ? header.slice(BEARER.length) : '';
};

/** Audits an MFA step for the caller, on the factor it named. */
function auditMfa(req, action: string, outcome = 'ok', factorId = req.body?.factor_id) {
  const subject = { actorKey: null, actorUser: req.user.id, targetType: 'mfa_factor', targetId: factorId || null };
  audit({ action, outcome, ...subject, req });
}

/**
 * Opens the caller's Supabase Auth client as req.mfa and sets req.user. A
 * "Login as" request is refused: an admin acting as a customer must never
 * add, pass or remove the customer's second factor.
 */
async function withMfa(req, res, next) {
  if (req.headers[IMPERSONATE_HEADER])
    return res.status(Status.FORBIDDEN).json({ error: 'Not available while logged in as someone else' });
  const opened = await openMfa(bearerOf(req), req.body?.refresh_token || readRefreshCookie(req)).catch((err) => {
    sendError(res, err, req);
  });
  if (!opened) return;
  Object.assign(req, { mfa: opened.auth, user: opened.user });
  next();
}

/** Runs one MFA step, answering its error as the step threw it. */
const step = (work: (req, res) => Promise<unknown>) => async (req, res) => {
  try {
    await work(req, res);
  } catch (err) {
    sendError(res, err, req);
  }
};

/** Answers 400 when the body does not name a factor and a six-digit code; true when it was answered. */
function badCode(req, res) {
  const { factor_id, code } = req.body || {};
  if (typeof factor_id === 'string' && factor_id && TOTP_CODE.test(String(code))) return false;
  res.status(Status.BAD_REQUEST).json({ error: 'Enter the 6-digit code from your authenticator app' });
  return true;
}

/** Answers 429 once this person has used up the hour's attempts, shared with password sign-in; true when it was answered. */
function verifyLimited(req, res) {
  const limit = consume('login', `mfa:${req.user.id}`);
  if (limit.allowed) return false;
  auditMfa(req, 'auth.mfa.verify', 'denied');
  res.set('Retry-After', String(limit.retryAfter));
  res.status(Status.TOO_MANY_REQUESTS).json({ error: 'Too many codes tried, try again later' });
  return true;
}

/** Checks the code, audits it either way, and starts the stepped-up session as a sign-in does. */
async function verify(req, res) {
  if (badCode(req, res) || verifyLimited(req, res)) return;
  const session = await verifyTotp(req.mfa, req.body.factor_id, String(req.body.code)).catch((err) => {
    auditMfa(req, 'auth.mfa.verify', 'denied');
    throw err;
  });
  auditMfa(req, 'auth.mfa.verify');
  issueSession(req, res, session);
}

/** Starts adding an authenticator and audits it under the new factor. */
async function enroll(req, res) {
  const enrolled = await enrollTotp(req.mfa);
  auditMfa(req, 'auth.mfa.enroll', 'ok', enrolled.id);
  res.json(enrolled);
}

/** Removes the named authenticator and audits it. */
async function unenroll(req, res) {
  if (typeof req.body?.factor_id !== 'string' || !req.body.factor_id)
    return res.status(Status.BAD_REQUEST).json({ error: 'factor_id required' });
  await unenrollTotp(req.mfa, req.body.factor_id);
  auditMfa(req, 'auth.mfa.unenroll');
  res.json({ ok: true });
}

/** Adds the MFA routes to the auth router. */
export function mfaRoutes(router: Router) {
  /** POST /auth/mfa/status, the caller's authenticators and this session's level (`aal1`, `aal2`). */
  router.post(
    '/auth/mfa/status',
    withMfa,
    step(async (req, res) => res.json(await mfaStatus(req.mfa))),
  );
  /** POST /auth/mfa/enroll, start adding an authenticator: its QR code, secret and factor id. */
  router.post('/auth/mfa/enroll', withMfa, step(enroll));
  /** POST /auth/mfa/verify {factor_id, code}, activate an authenticator or step the session up; answers a new session. */
  router.post('/auth/mfa/verify', withMfa, step(verify));
  /** POST /auth/mfa/unenroll {factor_id}, remove an authenticator (Supabase asks for an aal2 session for a verified one). */
  router.post('/auth/mfa/unenroll', withMfa, step(unenroll));
}
