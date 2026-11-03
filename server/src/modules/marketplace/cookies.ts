/** Host-only setup cookies keep trusted purchase context outside page-readable storage. */
import type { Request, Response } from 'express';
import { CONTINUATION_COOKIE, PENDING_COOKIE, CONTINUATION_MS, MAX_COOKIE } from './constants.ts';

/** Reads only the bounded continuation cookie; malformed escaping is an invalid context. */
export function setupCookie(req: Request): string {
  try {
    const value = setupPair(req)?.slice(CONTINUATION_COOKIE.length + 1) || '';
    return value.length <= MAX_COOKIE ? decodeURIComponent(value) : '';
  } catch {
    return '';
  }
}

/** Selects only the exact cookie name, ignoring unrelated session material. */
function setupPair(req: Request) {
  return (req.headers.cookie || '')
    .split(';')
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${CONTINUATION_COOKIE}=`));
}

/** Fresh cookies last only for the sealed context's fixed lifetime. */
export function writeSetupCookies(res: Response, value: string, origin: string) {
  const options = { path: '/', sameSite: 'lax' as const, secure: origin.startsWith('https:'), maxAge: CONTINUATION_MS };
  res.cookie(CONTINUATION_COOKIE, value, { ...options, httpOnly: true });
  res.cookie(PENDING_COOKIE, '1', { ...options, httpOnly: false });
}

/** Invalid callbacks must never retain the context of a previous purchase. */
export function clearSetupCookies(res: Response) {
  res.clearCookie(CONTINUATION_COOKIE, { path: '/' });
  res.clearCookie(PENDING_COOKIE, { path: '/' });
}

/** Prevents shared caching and referrer propagation of fulfillment responses. */
export function privateSetupResponse(res: Response) {
  res.set('Cache-Control', 'no-store');
  res.set('Referrer-Policy', 'no-referrer');
}
