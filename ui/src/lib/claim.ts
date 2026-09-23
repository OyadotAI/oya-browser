/**
 * A claim link an agent handed its person, remembered across sign-in. The
 * link carries the agent's key in its fragment; if nobody is signed in yet it
 * waits here (this tab only) while they sign in, with Google, GitHub or email,
 * and every sign-in then lands back on /claim instead of the dashboard.
 */

/** Where the pending key waits: sessionStorage, so it dies with the tab. */
const PENDING_CLAIM = 'oya_pending_claim';

/** Remembers a key to claim once someone is signed in. */
export function rememberClaim(key: string) {
  try {
    sessionStorage.setItem(PENDING_CLAIM, key);
  } catch {
    // Storage refused: the claim still works if they are already signed in.
  }
}

/** The key waiting to be claimed, or ''. */
export function pendingClaim(): string {
  try {
    return sessionStorage.getItem(PENDING_CLAIM) || '';
  } catch {
    return '';
  }
}

/** Forgets the pending key, once claimed or refused. */
export function forgetClaim() {
  try {
    sessionStorage.removeItem(PENDING_CLAIM);
  } catch {
    // Nothing stored to forget.
  }
}
