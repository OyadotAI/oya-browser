/** Private-cookie capabilities require explicit context ownership and implemented attribute semantics. */
import { cookieBatch, cookieURL, cookieString } from '../native-cookies/index.ts';
import type { NativeCommand } from './types.ts';
/** Standard Storage commands and one explicit URL/name deletion extension. */
export const COOKIE_PARAMS: Record<string, readonly string[]> = {
  'Storage.getCookies': ['browserContextId'],
  'Storage.setCookies': ['browserContextId', 'cookies'],
  'Storage.clearCookies': ['browserContextId'],
  'Oya.deleteCookie': ['browserContextId', 'url', 'name'],
};
/** Missing context never means the person's default profile. */
export function validateCookies({ method, params }: NativeCommand): void {
  if (!Object.hasOwn(COOKIE_PARAMS, method)) return;
  if (typeof params.browserContextId !== 'string' || !params.browserContextId)
    throw Error('An owned browserContextId is required');
  if (method === 'Storage.setCookies') cookieBatch(params.cookies);
  if (method === 'Oya.deleteCookie') {
    cookieURL(params.url);
    cookieString(params.name, true);
  }
}
