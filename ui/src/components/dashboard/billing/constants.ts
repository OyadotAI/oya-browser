/**
 * What the plan section offers: the paid plans, and one decimal for hours.
 */

/** The plans a person can upgrade to, as the section shows them. */
export const UPGRADES = [
  { id: 'developer', label: 'Developer', price: '$500/mo' },
  { id: 'startup', label: 'Startup', price: '$2,500/mo' },
];

/** New checkout rates never imply a change to an existing subscriber's charges. */
export const NEW_SUBSCRIPTION_PRICE_NOTE =
  'Prices shown apply to new subscriptions. Existing subscriptions keep their current prices.';

/** Where to write for more than the plans offer. */
export const SALES_EMAIL = 'sales@getoya.ai';

/** The billing page. */
export const BILLING_PAGE = '/dashboard/billing';
/** Bytes in a GB, as the pricing counts proxy traffic. */
export const BYTES_PER_GB = 1_073_741_824;
/** Micro-USD in a dollar. */
export const MICRO_USD_PER_DOLLAR = 1_000_000;
/** Cents in a dollar. */
export const CENTS_PER_DOLLAR = 100;

/** Hours are shown to one decimal. */
export const TENTHS = 10;
/** Digits after the point in money. */
export const CENT_DIGITS = 2;
/** A share as a percentage. */
export const PERCENT = 100;
