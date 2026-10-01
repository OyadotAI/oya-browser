/**
 * What the admin page shows: its limits and defaults.
 */

/** Installs listed on the page. */
export const INSTALLS_LISTED = 50;
/** A new license's default length, in days. */
export const DEFAULT_LICENSE_DAYS = 365;
/** A new license's default number of cloud browsers at once. */
export const DEFAULT_LICENSE_BROWSERS = 20;
/** Milliseconds in a day. */
export const MS_PER_DAY = 86_400_000;
/** Characters of an ISO time that make its day. */
export const DAY_CHARS = 10;
/** Seconds in an hour. */
export const SECONDS_PER_HOUR = 3600;
/** Characters of an install id shown: enough to tell installs apart. */
export const INSTALL_ID_CHARS = 8;
/** Oya staff's addresses: they get a link to the admin page. */
export const ADMIN_DOMAIN = '@getoya.ai';
/** Cents in a dollar. */
export const CENTS_PER_DOLLAR = 100;
/** A fraction as a percentage. */
export const PERCENT = 100;
/** Bars at least this tall, in percent of the chart, so a day with something on it never looks empty. */
export const MIN_BAR_PERCENT = 4;
