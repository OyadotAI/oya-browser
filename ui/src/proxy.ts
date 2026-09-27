/**
 * The request proxy (Next.js 16's name for middleware): gives every page
 * request a fresh nonce and the Content-Security-Policy it belongs to. The
 * policy itself, and why it is shaped the way it is, lives in lib/csp.ts.
 * It also answers an agent asking for /docs with `Accept: text/markdown`
 * with the Markdown twin, /docs.md.
 */
import { NextResponse, type NextRequest } from 'next/server';
import { contentSecurityPolicy } from '@/lib/csp';

/** Sets the nonce and CSP on the request (for the render) and on the response (for the browser). */
export function proxy(request: NextRequest) {
  const nonce = Buffer.from(crypto.randomUUID()).toString('base64');
  const csp = contentSecurityPolicy(nonce, process.env.NODE_ENV !== 'production');
  // The nonce reaches the render through the request headers; app/layout.tsx
  // reads it back with headers().get('x-nonce').
  const headers = new Headers(request.headers);
  headers.set('x-nonce', nonce);
  headers.set('content-security-policy', csp);
  const response = forward(request, headers);
  response.headers.set('content-security-policy', csp);
  return response;
}

/** /docs itself, the one page with a Markdown twin to negotiate. */
const isDocs = (request: NextRequest) => ['/docs', '/docs/'].includes(request.nextUrl.pathname);

/** On to the page, or to /docs.md for a client that asked for Markdown; /docs answers vary on Accept. */
function forward(request: NextRequest, headers: Headers) {
  if (!isDocs(request)) return NextResponse.next({ request: { headers } });
  const markdown = request.headers.get('accept')?.includes('text/markdown');
  const url = new URL('/docs.md', request.url);
  const response = markdown
    ? NextResponse.rewrite(url, { request: { headers } })
    : NextResponse.next({ request: { headers } });
  response.headers.set('vary', 'Accept');
  return response;
}

/** Which requests the proxy runs on. */
export const config = {
  matcher: [
    // Everything but the static assets, which carry no markup to inject into
    // and would only spend a nonce each.
    {
      source: '/((?!_next/static|_next/image|favicon.ico).*)',
      missing: [{ type: 'header', key: 'next-router-prefetch' }],
    },
  ],
};
