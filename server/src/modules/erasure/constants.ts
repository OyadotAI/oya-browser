/** Numbers and names erasure runs on. */

/** One hour in milliseconds. */
const HOUR_MS = 3_600_000;
/** One day in milliseconds. */
const DAY_MS = 86_400_000;

/**
 * How long a deleted project's data is kept before it is erased. Long enough
 * for its browsers to stop and their providers to be cleaned up (which needs
 * the project's sealed key); there is no undelete, so no longer than that.
 */
export const PURGE_GRACE_MS = DAY_MS;
/**
 * After the grace period every replica first drops what it holds in memory for
 * the project; storage is erased this much later, so no replica writes a row
 * back after it is gone.
 */
export const PURGE_SETTLE_MS = HOUR_MS;

/** Control-plane row kinds that carry a project and go with it. Recordings are erased by the recorder. */
export const PROJECT_KINDS = [
  'session',
  'credential',
  'membership',
  'webhook',
  'delivery',
  'invite',
  'ticket',
  'hold',
  'attachment',
  'idempotency',
  'slack_state',
];

/** Subscription states that still bill: an account in one cannot be deleted until it is cancelled. */
export const BILLING_STATES = ['active', 'trialing', 'past_due', 'unpaid', 'incomplete'];
