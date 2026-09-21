/**
 * Every number the control plane's HTTP surface and takeover flows run on, by
 * name.
 */

/** How long taking control waits for the agent's in-flight command to finish. */
export const ACQUIRE_WAIT_MS = 10_000;
/** Pause between attempts while a route waits for commands to settle. */
export const TRANSFER_RETRY_MS = 200;
/** Pause between attempts while the desktop waits for commands to settle. */
export const ACQUIRE_POLL_MS = 100;

/** The Authorization scheme prefix in front of an API key. */
export const BEARER_PREFIX = 'Bearer ';

/** Random bytes in an invitation code. */
export const INVITE_CODE_BYTES = 24;
/** How long an invitation stays redeemable: seven days, in seconds. */
export const INVITE_TTL_S = 604_800;
/** Milliseconds per second. */
export const MS_PER_SECOND = 1000;
/** Lifetime of a console credential minted for a signed-in user. */
export const CONSOLE_ACCESS_MS = 3_600_000;
