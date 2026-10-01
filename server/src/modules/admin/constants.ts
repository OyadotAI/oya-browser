/**
 * The numbers the admin page runs on.
 */
import { MS_PER_HOUR, MS_PER_MINUTE } from '../../platform/constants.ts';

/** Hours in a day. */
const HOURS_PER_DAY = 24;
/** Milliseconds in a day. */
export const MS_PER_DAY = HOURS_PER_DAY * MS_PER_HOUR;

/** Only people signed in with a confirmed address at this domain are admins. */
export const ADMIN_DOMAIN = '@getoya.ai';
/** Days of downloads and signups the overview shows. */
export const DAYS_SHOWN = 30;
/** Heaviest users the overview lists. */
export const TOP_USERS = 10;
/** Installs the overview lists, most recently seen first. */
export const INSTALLS_SHOWN = 200;
/** An install that pinged within this many days counts as active. */
export const ACTIVE_DAYS = 7;
/** Characters of an ISO time that make its UTC day. */
export const DAY_CHARS = 10;
/** Minutes between writes of the download counters. */
const FLUSH_MINUTES = 1;
/** How often download counters are written. */
export const DOWNLOAD_FLUSH_MS = FLUSH_MINUTES * MS_PER_MINUTE;
/** Days in the week the week-over-week numbers compare. */
export const WEEK_DAYS = 7;
/** Days counted as a month for monthly active people. */
export const MONTH_DAYS = 30;
/** A fraction as a percentage. */
export const PERCENT = 100;
/** How a person's own usage row is keyed: `u:<user id>` (platform/usage.ts personRow). */
export const PERSON_PREFIX = 'u:';
/** Months in a year, for a yearly price's monthly share. */
export const MONTHS_PER_YEAR = 12;
/** The most Stripe answers in one list call. ponytail: one page only, page through with starting_after past 100 subscriptions or invoices a month. */
export const STRIPE_PAGE = 100;
