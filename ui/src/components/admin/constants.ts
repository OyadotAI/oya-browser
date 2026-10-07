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
/** The admin page's tabs, in order: the id the URL carries (`?tab=`) and the label shown. */
export const TABS = [
  { id: 'overview', label: 'Overview' },
  { id: 'customers', label: 'Customers' },
  { id: 'self-hosted', label: 'Self-hosted' },
  { id: 'fleet', label: 'Fleet' },
] as const;
/** One tab's id. */
export type TabId = (typeof TABS)[number]['id'];

/** Micro-USD in a dollar of hosted model allowance. */
export const MICRO_USD_PER_DOLLAR = 1_000_000;
/** Precision for displayed dollar credits. */
export const CREDIT_DECIMALS = 2;
/** The longest adjustment explanation the server accepts. */
export const BILLING_REASON_MAX = 500;
/** Smallest dollar grant selectable in the form. */
export const CREDIT_STEP = 0.01;
/** Smallest cloud-hour grant selectable in the form. */
export const HOURS_STEP = 0.25;
