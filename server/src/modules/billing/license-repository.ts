/**
 * Where self-hosted licenses bought through Stripe are kept: rows of the same
 * licenses table admins issue into, so the admin page lists and revokes them
 * alike. Unlike an admin's, a bought license keeps its key, sealed, so its
 * buyer can see it again.
 */
import { getConnection } from '../../platform/storage/index.ts';

/** The table's name. */
const TABLE = 'licenses';

/** A license row, as stored. */
export type LicenseRow = Record<string, any> & {
  /** Its id, which the install reports in its ping. */
  id: string;
};

/** The license already issued for a Stripe invoice, or null: what makes a replayed webhook mint nothing. */
export async function byInvoice(invoiceId: string): Promise<LicenseRow | null> {
  const [row] = await getConnection().select(TABLE, { stripe_invoice_id: invoiceId });
  return (row as LicenseRow) || null;
}

/** A person's bought licenses that are not revoked, newest first. */
export const forUser = (userId: string): Promise<LicenseRow[]> =>
  getConnection().select(TABLE, { user_id: userId, revoked_at: null }, { order: ['created_at', 'desc'] }) as Promise<
    LicenseRow[]
  >;

/** Stores a newly issued license; a second row for the same invoice fails on the unique index rather than landing. */
export const insert = (row: LicenseRow) => getConnection().upsert(TABLE, [row]);

/** Whether any license bought under a Stripe subscription was revoked: a renewal on it issues no new key. */
export async function revokedUnder(subscriptionId: string) {
  const rows = await getConnection().select(TABLE, { stripe_subscription_id: subscriptionId });
  return rows.some((row) => row.revoked_at);
}

/** Revokes every live license bought under a Stripe subscription; answers how many. */
export const revokeUnder = (subscriptionId: string, at: string) =>
  getConnection().update(TABLE, { stripe_subscription_id: subscriptionId, revoked_at: null }, { revoked_at: at });
