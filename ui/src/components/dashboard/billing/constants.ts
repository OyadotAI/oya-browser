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

/** The hash the server's upgrade links end in; the dashboard opens the account dialog on it. */
export const BILLING_HASH = '#billing';

/** Hours are shown to one decimal. */
export const TENTHS = 10;
