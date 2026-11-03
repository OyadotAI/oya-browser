/** Immutable buyer ownership and license registration survive retries, restarts and SQL replicas. */
import { getConnection, type Connection } from '../../platform/storage/index.ts';
import { ownershipConflict, type BuyerIdentity } from './model.ts';

/** The account-wide owner prevents multiple licenses being claimed by different people. */
const ACCOUNTS = 'aws_marketplace_accounts';
/** A registration is deliberately separate from any paid entitlement or Stripe row. */
const REGISTRATIONS = 'aws_marketplace_registrations';

/** Durable association surface injected into the registration service. */
export interface RegistrationRepository {
  /** Links only after checking an immutable account owner. */
  connect(identity: BuyerIdentity, userId: string, now: number): Promise<void>;
  /** Reports a license association only to the account's actual owner. */
  registered(identity: BuyerIdentity, userId: string): Promise<boolean>;
}

/** Storage-driver-independent registration with insert-if-absent ownership. */
export class MarketplaceRepository implements RegistrationRepository {
  /** Late storage access preserves test isolation and production driver selection. */
  constructor(privateConnection: () => Connection = getConnection) {
    this.connection = privateConnection;
  }
  /** The repository alone accesses persistence. */
  private connection: () => Connection;

  /** Reserves the account first; the same owner can finish after a partial write. */
  async connect(identity: BuyerIdentity, userId: string, now: number) {
    const db = this.connection();
    const row = { aws_account_id: identity.accountId, user_id: userId, linked_at: new Date(now).toISOString() };
    await db.upsert(ACCOUNTS, [row]);
    await this.requireOwner(identity.accountId, userId);
    await db.upsert(REGISTRATIONS, [this.registration(identity, now)]);
    await this.registered(identity, userId);
  }

  /** A read never creates or transfers an association. */
  async registered(identity: BuyerIdentity, userId: string) {
    const owner = await this.requireOwner(identity.accountId, userId);
    const [row] = await this.connection().select(REGISTRATIONS, { license_arn: identity.licenseArn });
    if (row && (row.aws_account_id !== identity.accountId || row.product_code !== identity.productCode))
      throw ownershipConflict();
    return Boolean(owner && row);
  }

  /** Missing ownership is fine on a read; a different owner is always a conflict. */
  private async requireOwner(accountId: string, userId: string) {
    const [owner] = await this.connection().select(ACCOUNTS, { aws_account_id: accountId });
    if (owner && owner.user_id !== userId) throw ownershipConflict();
    return owner;
  }

  /** Inserts no status that could be mistaken for AWS activation evidence. */
  private registration(identity: BuyerIdentity, now: number) {
    return {
      license_arn: identity.licenseArn,
      aws_account_id: identity.accountId,
      product_code: identity.productCode,
      registered_at: new Date(now).toISOString(),
    };
  }
}
