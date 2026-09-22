/**
 * Content-Security-Policy, with a per-request nonce.
 *
 * The console holds an administrator credential for a browser fleet, so the
 * cost of any XSS here is the fleet. `script-src 'self' 'nonce-…'` is the
 * directive that actually raises that cost: an injected `<script>` has no
 * nonce, and neither does an injected `onclick`. It has to be set in the proxy
 * rather than in next.config.ts because the nonce changes per request, and the
 * two inline scripts in app/layout.tsx read it back from this header.
 *
 * `connect-src` has to be computed too: NEXT_PUBLIC_API_URL may point at a
 * different origin than the console, and hard-coding 'self' would silently
 * break every request in that topology. POSTHOG_HOST joins it only when the
 * operator set one, so a console with analytics off can reach nothing extra.
 *
 * `style-src` keeps 'unsafe-inline'. Next.js emits inline <style> for fonts and
 * critical CSS, and there is no style equivalent of the nonce plumbing above
 * that survives streaming. Inline styles are not script execution, and
 * `base-uri`/`object-src`/`form-action` below close the usual ways of turning
 * one into the other.
 */

/** Directives before script-src and connect-src, which depend on the request. */
const OPENING = ["default-src 'self'"];

/** Directives between script-src and connect-src. */
const MIDDLE = [
  "style-src 'self' 'unsafe-inline'",
  // data: for the live view's JPEG frames, blob: for anything captured client-side.
  "img-src 'self' data: blob:",
  "media-src 'self' blob:",
  "font-src 'self' data:",
];

/** Directives after connect-src. */
const CLOSING = ["frame-ancestors 'none'", "object-src 'none'", "base-uri 'self'", "form-action 'self'"];

/** Where the page may send requests: itself, the API's origin when elsewhere, and the dev bundler's socket. */
function connectSources(dev: boolean): string {
  return ["'self'", originOf(process.env.NEXT_PUBLIC_API_URL), originOf(process.env.POSTHOG_HOST), dev ? 'ws:' : null]
    .filter(Boolean)
    .join(' ');
}

/** The origin of a URL the page may send to, or null when the setting is absent or not a URL. */
function originOf(url: string | undefined): string | null {
  return url?.startsWith('http') ? new URL(url).origin : null;
}

/**
 * Scripts that may run. 'unsafe-eval' in development only: the dev bundler
 * needs it, production does not, and shipping it would give most of the nonce back.
 */
function scriptSources(nonce: string, dev: boolean): string {
  return `script-src 'self' 'nonce-${nonce}' 'strict-dynamic'${dev ? " 'unsafe-eval'" : ''}`;
}

/** The full policy for one response. */
export function contentSecurityPolicy(nonce: string, dev: boolean): string {
  const directives = [...OPENING, scriptSources(nonce, dev), ...MIDDLE, `connect-src ${connectSources(dev)}`];
  return [...directives, ...CLOSING].join('; ');
}
