/**
 * Where a person enters the code from their authenticator app, and the way
 * back. A session that owes its second factor is sent to the code page from
 * anywhere in the console, and an admin page refused with `mfa_required` sends
 * the person there too; either way they come back to where they were.
 */

/** The page that sets up, checks and removes authenticator apps. */
export const MFA_PAGE = '/account/mfa';

/** The code page, coming back to `next` once the code is accepted. */
export const stepUpUrl = (next: string) => `${MFA_PAGE}?next=${encodeURIComponent(next)}`;

/** Control characters (which browsers strip from URLs, turning `/\t/host` into `//host`) and backslashes. */
const UNSAFE_URL_CHARS = /[\u0000-\u001f\u007f\\]/;

/**
 * Where to go after the code page: the path, query and fragment of `raw` when
 * it resolves to this site, else nowhere. It is parsed, not prefix-checked, so
 * no `//host`, absolute URL or stripped-character trick can send a freshly
 * signed-in person to another site.
 */
export function safeNext(raw: string | null): string {
  if (!raw || !raw.startsWith('/') || UNSAFE_URL_CHARS.test(raw)) return '';
  try {
    const url = new URL(raw, window.location.origin);
    return url.origin === window.location.origin ? url.pathname + url.search + url.hash : '';
  } catch {
    return '';
  }
}

/** Whether `pathname` is the code page itself, which must never send a person to itself. */
export const onMfaPage = (pathname: string) => pathname === MFA_PAGE;
