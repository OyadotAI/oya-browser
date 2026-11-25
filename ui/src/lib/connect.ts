/**
 * An app's request to connect (claude.ai signing in to Oya over OAuth),
 * remembered across sign-in. The request id arrives in the /connect link; if
 * nobody is signed in yet it waits here (this tab only) while they sign in, and
 * every sign-in then lands back on /connect instead of the dashboard. The
 * project last connected is kept too, so the next connection starts on it.
 */

/** Where the pending request waits: sessionStorage, so it dies with the tab. */
const PENDING_CONNECT = 'oya_pending_connect';

/** Remembers a request to answer once someone is signed in. */
export function rememberConnect(id: string) {
  try {
    sessionStorage.setItem(PENDING_CONNECT, id);
  } catch {
    // Storage refused: the request still works if they are already signed in.
  }
}

/** The request waiting to be answered, or ''. */
export function pendingConnect(): string {
  try {
    return sessionStorage.getItem(PENDING_CONNECT) || '';
  } catch {
    return '';
  }
}

/** Forgets the pending request, once answered. */
export function forgetConnect() {
  try {
    sessionStorage.removeItem(PENDING_CONNECT);
  } catch {
    // Nothing stored to forget.
  }
}

/** Where the last project an app was connected to is kept: a convenience for this browser only. */
const LAST_PROJECT = 'oya_connect_project';

/** Remembers the project just connected, so the next connection starts on it. */
export function rememberProject(id: string) {
  try {
    localStorage.setItem(LAST_PROJECT, id);
  } catch {
    // Storage refused: the next connection starts on the first project instead.
  }
}

/** The project last connected from this browser, or ''. */
export function lastProject(): string {
  try {
    return localStorage.getItem(LAST_PROJECT) || '';
  } catch {
    return '';
  }
}
