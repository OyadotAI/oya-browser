/**
 * Opens the installed desktop browser signed in with this key, through a
 * single-use pairing link (see `desktopSignInUrl`). A failure is toasted. A
 * link no app answers (the browser is not installed) is noticed, since the
 * web page cannot ask whether an app is installed.
 */
import { useState } from 'react';
import { desktopSignInUrl } from '../config';
import { useBusyAction } from './use-busy-action';
import { APP_OPEN_WAIT_MS } from './constants';
import type { DesktopReturn } from '../desktop-return';

/** `open(profile?)` hands the desktop a pairing link; `busy` while the link is made; `notOpened` when no app took it. */
export function useDesktopSignIn(apiKey: string) {
  const { busy, run } = useBusyAction();
  const [notOpened, setNotOpened] = useState(false);
  const open = (profile?: string, returnTo?: DesktopReturn) => run(() => pair(apiKey, profile, setNotOpened, returnTo));
  return { busy, open, notOpened };
}

/** Follows a fresh pairing link, then watches whether an app took it. */
async function pair(
  apiKey: string,
  profile: string | undefined,
  setNotOpened: (on: boolean) => void,
  returnTo?: DesktopReturn,
) {
  setNotOpened(false);
  window.location.href = await signInAddress(apiKey, profile, returnTo);
  if (!returnTo) watchAppOpened(() => setNotOpened(true));
}

/** Legacy callers retain their existing argument contract during a coordinated rollout. */
function signInAddress(apiKey: string, profile: string | undefined, returnTo?: DesktopReturn) {
  return returnTo ? desktopSignInUrl(apiKey, profile, returnTo) : desktopSignInUrl(apiKey, profile);
}

/** Calls `missing` when the page keeps focus for APP_OPEN_WAIT_MS: an app that opened would have taken it. */
function watchAppOpened(missing: () => void) {
  const stop = () => {
    clearTimeout(timer);
    window.removeEventListener('blur', stop);
    document.removeEventListener('visibilitychange', stop);
  };
  const timer = setTimeout(() => (stop(), missing()), APP_OPEN_WAIT_MS);
  window.addEventListener('blur', stop);
  document.addEventListener('visibilitychange', stop);
}
