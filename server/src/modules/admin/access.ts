/**
 * Who may open the admin page: a signed-in admin (auth/admins.ts decides who
 * that is), with a second factor when OYA_ADMIN_REQUIRE_MFA is on. Anyone else
 * is told no, without saying what is there.
 */
import { Status } from '../../platform/http-status.ts';
import { isAdmin, adminMfaRequired, MFA_AAL } from '../auth/service.ts';

export { isAdmin };

/** Runs after userAuthMiddleware: lets admins through, answers 403 to anyone else. */
export function adminOnly(req, res, next) {
  if (!isAdmin(req.user)) return res.status(Status.FORBIDDEN).json({ error: 'Admins only', code: 'forbidden' });
  if (adminMfaRequired() && req.authAal !== MFA_AAL)
    return res.status(Status.FORBIDDEN).json({ error: 'Sign in with a second factor', code: 'mfa_required' });
  next();
}
