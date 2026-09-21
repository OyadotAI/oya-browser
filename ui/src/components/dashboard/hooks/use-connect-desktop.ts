/**
 * The desktop's "Sign in" button opens `/dashboard?connect=desktop`. The
 * request is remembered across login or signup, then answered with the pairing
 * link as soon as the console has a key, so a first-time user never copies a key.
 */
import { useEffect } from 'react';
import { useDesktopSignIn } from './use-desktop-sign-in';

/** Where the pending request is kept; localStorage survives an email-confirmation tab. */
const PENDING = 'oya.connectDesktop';

/** Remembers a `?connect=desktop` visit; call it before any redirect to /login. */
export function rememberDesktopConnect(): void {
  if (new URLSearchParams(window.location.search).get('connect') !== 'desktop') return;
  window.localStorage.setItem(PENDING, '1');
}

/** Once `apiKey` is known, answers a remembered request with the pairing link, once. */
export function useConnectDesktop(apiKey: string): void {
  const { open } = useDesktopSignIn(apiKey);
  useEffect(() => {
    if (!apiKey || !window.localStorage.getItem(PENDING)) return;
    window.localStorage.removeItem(PENDING);
    void open();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- open is rebuilt each render; the key is the trigger
  }, [apiKey]);
}
