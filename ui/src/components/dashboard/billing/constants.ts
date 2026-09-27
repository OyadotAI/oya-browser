/**
 * What the plan section offers: the paid plans, and one decimal for hours.
 */

/** The plans a person can upgrade to, as the section shows them. */
export const UPGRADES = [
  { id: 'developer', label: 'Developer', price: '$20/mo' },
  { id: 'startup', label: 'Startup', price: '$99/mo' },
];

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
