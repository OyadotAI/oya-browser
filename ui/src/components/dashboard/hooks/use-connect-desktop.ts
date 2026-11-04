/**
 * The desktop's "Sign in" button opens `/dashboard?connect=desktop`. The
 * request is remembered across login or signup, then answered with the pairing
 * link as soon as the console has a key, so a first-time user never copies a key.
 */
import { useEffect } from 'react';
import { useDesktopSignIn } from './use-desktop-sign-in';
import { desktopReturnFrom, validDesktopReturn, type DesktopReturn } from '../desktop-return';

/** Where the pending request is kept; localStorage survives an email-confirmation tab. */
const PENDING = 'oya.connectDesktop';

/** Remembers a `?connect=desktop` visit; call it before any redirect to /login. */
export function rememberDesktopConnect(): void {
  const params = new URLSearchParams(window.location.search);
  if (params.get('connect') !== 'desktop') return;
  const request = desktopReturnFrom(params);
  if (request) window.localStorage.setItem(PENDING, JSON.stringify(request));
  else if (!params.has('callback') && !params.has('state')) window.localStorage.setItem(PENDING, '1');
  else window.localStorage.removeItem(PENDING);
  clearReturnQuery(params);
}

/** Remove temporary return data before auth redirects or unrelated page requests. */
function clearReturnQuery(params: URLSearchParams): void {
  for (const name of ['connect', 'callback', 'state']) params.delete(name);
  window.history.replaceState(
    null,
    '',
    `${window.location.pathname}${params.size ? `?${params}` : ''}${window.location.hash}`,
  );
}

/** Old dashboards used a flag; new requests expire instead of replaying across later logins. */
function pendingReturn(raw: string): DesktopReturn | undefined | null {
  if (raw === '1') return undefined;
  try {
    const value = JSON.parse(raw);
    return validDesktopReturn(value) ? value : null;
  } catch {
    return null;
  }
}

/** Once `apiKey` is known, answers a remembered request with the pairing link, once. */
export function useConnectDesktop(apiKey: string): void {
  const { open } = useDesktopSignIn(apiKey);
  useEffect(() => {
    const raw = window.localStorage.getItem(PENDING);
    if (!apiKey || !raw) return;
    window.localStorage.removeItem(PENDING);
    const request = pendingReturn(raw);
    if (request !== null) void open(undefined, request);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- open is rebuilt each render; the key is the trigger
  }, [apiKey]);
}
