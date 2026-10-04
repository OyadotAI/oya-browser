/**
 * Who is an Oya administrator. With OYA_ADMIN_EMAILS set, exactly the confirmed
 * addresses it lists; without it, any confirmed address at the company domain,
 * so a deploy that has not set the list yet is not locked out. The admin page
 * and "Login as" both ask here, and "Login as" asks again on every request.
 */
import { ADMIN_DOMAIN } from './constants.ts';

/** The configured admin addresses, lowercased; empty when OYA_ADMIN_EMAILS is unset. */
export const adminEmails = (env = process.env) =>
  String(env.OYA_ADMIN_EMAILS || '')
    .split(',')
    .map((e) => e.trim().toLowerCase())
    .filter(Boolean);

/** Whether admins must have signed in with a second factor (OYA_ADMIN_REQUIRE_MFA=true). */
export const adminMfaRequired = (env = process.env) => env.OYA_ADMIN_REQUIRE_MFA === 'true';

/** Whether an address is at the admin domain, with exactly one @. */
const atDomain = (email: string) => email.endsWith(ADMIN_DOMAIN) && email.indexOf('@') === email.lastIndexOf('@');

/** Whether a signed-in user is an admin: a confirmed address on the list, or at the domain when there is no list. */
export function isAdmin(user) {
  const email = String(user?.email || '').toLowerCase();
  if (!user?.email_confirmed_at || !email) return false;
  const listed = adminEmails();
  return listed.length ? listed.includes(email) : atDomain(email);
}
