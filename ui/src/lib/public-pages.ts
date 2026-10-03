/**
 * Which pages are public: the landing page, docs, sign-in and sign-up. Visitor
 * tracking (RB2B, Meta's pixel, click tracking) runs only there. The console
 * and the live view show API keys and live browser frames, the admin page
 * shows customers, and a claim link and the OAuth callback carry a key or a
 * token in their URL, so nothing third-party loads on any of them. (The
 * callback reports a new account to Meta itself, after clearing its URL.)
 */

/** Path prefixes of pages no third party may see. */
const PRIVATE_PREFIXES = ['/dashboard', '/live', '/admin', '/claim', '/auth/callback'];

/** Whether `pathname` is a public page. Matches a prefix as a whole segment, so `/dashboards` would still be public. */
export function isPublicPage(pathname: string) {
  return !PRIVATE_PREFIXES.some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));
}
