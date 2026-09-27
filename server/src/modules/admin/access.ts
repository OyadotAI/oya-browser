/**
 * Who may open the admin page: a signed-in person whose confirmed email is at
 * the company's domain. Anyone else is told no, without saying what is there.
 */
import { Status } from '../../platform/http-status.ts';
import { ADMIN_DOMAIN } from './constants.ts';

/** Whether a signed-in user is an admin: a confirmed address at the admin domain. */
export function isAdmin(user) {
  const email = String(user?.email || '').toLowerCase();
  return (
    Boolean(user?.email_confirmed_at) && email.endsWith(ADMIN_DOMAIN) && email.indexOf('@') === email.lastIndexOf('@')
  );
}

/** Runs after userAuthMiddleware: lets admins through, answers 403 to anyone else. */
export function adminOnly(req, res, next) {
  if (isAdmin(req.user)) return next();
  res.status(Status.FORBIDDEN).json({ error: 'Admins only', code: 'forbidden' });
}
