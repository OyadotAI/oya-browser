/**
 * Numbers the browser providers and the cloud sandbox run on, by name.
 * The CDP driver keeps its own in cdp/constants.ts.
 */

/** How long a vendor may take to create a session. */
export const PROVIDER_CREATE_TIMEOUT_MS = 30_000;
/** How long a vendor may take to release a session. */
export const PROVIDER_RELEASE_TIMEOUT_MS = 15_000;

/** Hex characters of the owner tag labelled on a sandbox. */
export const OWNER_TAG_CHARS = 32;
/** Characters of a browser id used in its default display name. */
export const SHORT_ID_CHARS = 8;
/** Shortest idle stop a deployment may configure, in minutes. */
export const MIN_SANDBOX_TTL_MINUTES = 5;
/** Idle stop when none is configured, in minutes. */
export const DEFAULT_SANDBOX_TTL_MINUTES = 60;
/** The hard TTL runs this many minutes past the idle stop. */
export const SANDBOX_TTL_GRACE_MINUTES = 10;
/** Seconds the runtime may take to create a sandbox. */
export const SANDBOX_CREATE_TIMEOUT_S = 90;
/** Seconds the runtime may take to delete a sandbox. */
export const SANDBOX_DELETE_TIMEOUT_S = 60;
/** Keys whose sandbox inventory is cached at once; the oldest is dropped past this. */
export const INVENTORY_MAX_KEYS = 500;
/** How long a key's sandbox inventory is reused before it is refreshed. */
export const INVENTORY_TTL_MS = 5000;
/** How long a listing waits on a refresh before answering with what it has. */
export const INVENTORY_WAIT_MS = 3000;
