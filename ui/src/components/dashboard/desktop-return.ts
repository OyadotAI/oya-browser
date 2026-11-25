/** Validate the native app's bounded loopback return without allowing arbitrary redirects. */
import { DESKTOP_RETURN_TTL_MS, DESKTOP_STATE_SHAPE } from './hooks/constants';

/** No credential is stored here: the app redeems a later one-use pairing code itself. */
export interface DesktopReturn {
  /** Literal local callback with an OS-assigned port. */ callback: string;
  /** Unpredictable state owned by the initiating app. */ state: string;
  /** Expired browser requests must not issue a new pairing code. */ expiresAt: number;
}
/** Only the app's fixed path on literal loopback can receive the return. */
export function validDesktopReturn(value: unknown, now = Date.now()): value is DesktopReturn {
  if (!value || typeof value !== 'object') return false;
  const v = value as DesktopReturn;
  const url = typeof v.callback === 'string' ? URL.parse(v.callback) : null;
  return !!url && isLoopback(url) && validState(v, now);
}
/** Loopback is literal and cannot carry a nested redirect or embedded credentials. */
function isLoopback(url: URL): boolean {
  const address = url.protocol === 'http:' && url.hostname === '127.0.0.1' && !!url.port;
  return (
    address && url.pathname === '/oya-desktop/callback' && !url.username && !url.password && !url.search && !url.hash
  );
}
/** Time-bounded native state cannot survive a later unrelated web login. */
function validState(v: DesktopReturn, now: number): boolean {
  return (
    typeof v.state === 'string' &&
    DESKTOP_STATE_SHAPE.test(v.state) &&
    Number.isFinite(v.expiresAt) &&
    v.expiresAt > now &&
    v.expiresAt <= now + DESKTOP_RETURN_TTL_MS
  );
}
/** A new dashboard visit may remember only a validated, short-lived return request. */
export function desktopReturnFrom(params: URLSearchParams): DesktopReturn | null {
  const value = {
    callback: params.get('callback'),
    state: params.get('state'),
    expiresAt: Date.now() + DESKTOP_RETURN_TTL_MS,
  };
  return validDesktopReturn(value) ? value : null;
}
/** Add only a one-use code and state, never the workspace key or a caller-supplied server. */
export function desktopReturnUrl(request: DesktopReturn, code: string): string {
  if (!validDesktopReturn(request) || !code) throw Error('Desktop sign-in expired. Start again in Oya.');
  const url = new URL(request.callback);
  url.search = new URLSearchParams({ code, state: request.state }).toString();
  return url.href;
}
