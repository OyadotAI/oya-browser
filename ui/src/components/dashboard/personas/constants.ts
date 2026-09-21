/**
 * Every number the profile screens run on, by name: debounce, form defaults
 * and display limits.
 */

/** Pause after the last device choice before asking the server for a preview. */
export const PREVIEW_DEBOUNCE_MS = 150;
/** Concurrent browsers a new profile allows unless changed: a phone and a laptop. */
export const DEFAULT_CONCURRENT = '2';
/** Longest proxy geo hint accepted (a country or region code). */
export const GEO_MAX_LENGTH = 8;
/** Running browsers listed in the drawer before the rest collapse into "and N more". */
export const RUNNING_LISTED = 6;
/** Longest display name the account form accepts. */
export const DISPLAY_NAME_MAX_LENGTH = 100;
/** Columns in the proxies table, so the empty-state row spans them all. */
export const PROXY_TABLE_COLUMNS = 6;
/** Profiles a new proxy serves unless changed: one, for a sticky session. */
export const DEFAULT_MAX_PROFILES = '1';
