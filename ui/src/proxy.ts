/**
 * The request proxy (Next.js 16's name for middleware): gives every page
 * request a fresh nonce and the Content-Security-Policy it belongs to. The
 * policy itself, and why it is shaped the way it is, lives in lib/csp.ts.
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
  const response = NextResponse.next({ request: { headers } });
  response.headers.set('content-security-policy', csp);
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
