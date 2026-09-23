/**
 * Which pages are public: the landing page, docs, sign-in and sign-up. Visitor
 * tracking (RB2B, click tracking) runs only there. The console and the live
 * view show API keys and live browser frames, so nothing third-party runs on them.
 */

/** Path prefixes of the signed-in console. */
const PRIVATE_PREFIXES = ['/dashboard', '/live'];

/** Whether `pathname` is a public page. Matches a prefix as a whole segment, so `/dashboards` would still be public. */
export function isPublicPage(pathname: string) {
  return !PRIVATE_PREFIXES.some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));
}
