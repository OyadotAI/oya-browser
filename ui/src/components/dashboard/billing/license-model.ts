/**
 * What the self-hosted license section reckons: the browsers it may sell, what
 * they cost each period, and what the person's license says. The server holds
 * the same bounds; these only spare a round trip.
 */

/** The fewest browsers sold: up to 5 run free. */
export const LICENSE_MIN = 6;
/** The most browsers sold here; above it, sales. */
export const LICENSE_MAX = 50;
/** Dollars per browser for each billing period. */
export const LICENSE_PRICES = { month: 20, year: 240 } as const;
/** A billing period. */
export type LicenseInterval = keyof typeof LICENSE_PRICES;
/** How often a pending key is looked for, in ms. */
export const LICENSE_POLL_MS = 3000;
/** How long a pending key is looked for before the page says it is slow, in ms. */
export const LICENSE_SLOW_MS = 30_000;
/** The section's anchor, which the server's 402 links to. */
export const LICENSE_ANCHOR = 'self-hosted';
/** How long "Copied" shows, in ms. */
export const COPIED_MS = 2000;

/** A self-hosted license, as the server shows it to its buyer. */
export interface License {
  /** Its id. */
  id: string;
  /** Cloud browsers at once. */
  browsers: number;
  /** month or year. */
  interval: LicenseInterval;
  /** The key to set as OYA_LICENSE_KEY. */
  key: string;
  /** When the key stops working. */
  expiresAt: string;
  /** When the key was issued. */
  issuedAt: string;
  /** Stripe's subscription status, or null when Stripe could not say. */
  status: string | null;
  /** Whether it is set to end at the period's close. */
  cancelAtPeriodEnd: boolean;
  /** When it renews, or null when it will not. */
  renewsAt: string | null;
  /** When the previous key stops, while it still works after a renewal. */
  previousExpiresAt: string | null;
}

/** Why a browser count cannot be bought, or '' when it can. */
export function quantityProblem(browsers: number) {
  if (!Number.isInteger(browsers) || browsers < LICENSE_MIN) return 'Up to 5 run free. Choose 6 or more.';
  if (browsers > LICENSE_MAX) return 'over';
  return '';
}

/** The total for a count and period, as the form says it: "$120 a month". */
export const totalOf = (browsers: number, interval: LicenseInterval) =>
  `$${(browsers * LICENSE_PRICES[interval]).toLocaleString('en-US')} a ${interval}`;

/** Which state a license is in, for what the section says about it. */
export function licenseState(license: License, now = Date.now()) {
  if (new Date(license.expiresAt).getTime() <= now) return 'expired';
  if (license.status === 'past_due' || license.status === 'unpaid') return 'past_due';
  if (license.cancelAtPeriodEnd || license.status === 'canceled') return 'cancelled';
  return 'active';
}
