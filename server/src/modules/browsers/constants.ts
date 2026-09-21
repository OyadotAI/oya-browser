/**
 * Every number the browsers module runs on outside the connection itself, by
 * name: health thresholds, activity and viewer limits, relay and route limits.
 */

/** How many recent commands each browser remembers. Enough to see what it is doing. */
export const ACTIVITY_SIZE = 50;
/** Recent commands looked at when judging whether a browser is failing. */
export const HEALTH_WINDOW = 10;
/** Failures among those recent commands that mark a browser as in trouble. */
export const FAILING_COMMANDS = 3;
/** Silence after which an inbound browser is dead. */
export const DEAD_AFTER_MS = 80_000;
/** Silence after which an inbound browser is stale. */
export const STALE_AFTER_MS = 40_000;
/** Longest URL an activity entry keeps. */
export const SUMMARY_CHARS = 200;
/** Longest error message an activity entry keeps. */
export const ERROR_CHARS = 200;
/** A live-view viewer more than this many bytes behind skips frames (1 MiB). */
export const VIEWER_BACKLOG_BYTES = 1_048_576;

/** How long a browser has to confirm a CDP relay before it is abandoned. */
export const RELAY_OPEN_TIMEOUT_MS = 20_000;

/** WebSocket close code for a browser an operator stopped or disconnected. */
export const OPERATOR_CLOSE = 4008;
/** Longest browser display name accepted. */
export const MAX_NAME = 100;
/** Most ids one bulk stop accepts. */
export const MAX_STOP_IDS = 5000;
/** Browsers a bulk stop stops at once. */
export const STOP_BATCH = 8;
/** Most sandboxes one provision call launches. */
export const MAX_PROVISION = 100;
/** Sandbox creates one provision call keeps in flight. */
export const PROVISION_IN_FLIGHT = 10;
/** The port a CDP URL names when the request carries no Host header. */
export const DEFAULT_PORT = 3100;
/** Cookies accepted by one PUT /pool/cookies; a whole browser profile fits, a runaway upload does not. */
export const MAX_IMPORT_COOKIES = 20_000;
