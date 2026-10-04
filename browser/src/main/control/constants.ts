/**
 * Numbers for the control state: how long a request to the server may take,
 * how a takeover waits for local commands, and when human control is renewed.
 */

/** A control request the server has not answered by then fails. */
export const CONTROL_REQUEST_TIMEOUT_MS = 12_000;

/** Longest a takeover waits for local automation commands to finish. */
export const LOCAL_DRAIN_TIMEOUT_MS = 10_000;

/** How often a takeover checks whether local commands have finished. */
export const LOCAL_DRAIN_POLL_MS = 50;

/** How often human control is checked for expiry or renewal. */
export const CONTROL_TICK_MS = 1000;

/** Human control is renewed once it has less than this left. */
export const CONTROL_RENEW_BEFORE_MS = 240_000;
