/**
 * Every number the fleet routes run on, by name: batch and page limits and
 * default windows.
 */

/** Most API keys one provisioning call may mint. */
export const MAX_PROVISION = 10000;
/** Hours of usage history returned when the caller names none. */
export const DEFAULT_USAGE_HOURS = 24;
/** Audit rows returned when the caller names no limit. */
export const DEFAULT_AUDIT_LIMIT = 100;
/** Most audit rows one call may return. */
export const MAX_AUDIT_LIMIT = 1000;
