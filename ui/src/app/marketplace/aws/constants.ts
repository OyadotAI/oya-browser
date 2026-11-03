/** Buyer-facing actions reuse the published Oya contact and existing auth styling. */

/** Existing public Oya contact, used for account-setup recovery. */
export const SUPPORT_URL = 'mailto:sales@getoya.ai';
/** Buyers return to their existing purchase, rather than being asked to pay again. */
export const AWS_MARKETPLACE_URL = 'https://aws.amazon.com/marketplace';
/** Existing auth primary action with a minimum touch target. */
export const PRIMARY =
  'flex min-h-11 w-full items-center justify-center rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-accent-foreground hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-60';
/** Secondary controls remain accessible without introducing a new design system. */
export const SECONDARY =
  'flex min-h-11 w-full items-center justify-center text-sm font-medium text-accent hover:text-accent-hover';
