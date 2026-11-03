/** Registration owns sequencing; resolved or linked purchases remain pending until AWS activation exists. */
import { invalidSetup, validIdentity, type SetupState } from './model.ts';
import type { ResolveToken } from './aws.ts';
import type { MarketplaceConfig } from './config.ts';
import type { RegistrationRepository } from './repository.ts';
import { openContext, sealContext } from './continuation.ts';
import { ACCOUNT_SUFFIX } from './constants.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';

/** All external state and timing is injected at the composition root. */
export interface MarketplaceDeps {
  /** Explicit opt-in configuration, read at operation time. */
  config(): MarketplaceConfig;
  /** Seller-owned token exchange. */
  resolve: ResolveToken;
  /** Durable immutable ownership. */
  repository: RegistrationRepository;
  /** Absolute clock for continuation expiry and persistence. */
  now(): number;
}

/** The account handoff, without authority to activate paid access. */
export class MarketplaceService {
  /** Dependencies never reach through to another process-wide service. */
  private deps: MarketplaceDeps;
  /** Builds one registration service over explicit dependencies. */
  constructor(deps: MarketplaceDeps) {
    this.deps = deps;
  }

  /** Resolves and seals immediately so authentication does not consume the AWS token's lifetime. */
  async begin(token: string) {
    const config = this.deps.config();
    const identity = await this.deps.resolve(token, config.region);
    if (!validIdentity(identity, config.productCode)) throw invalidSetup();
    return sealContext(identity, this.deps.now());
  }

  /** Anonymous reads disclose only a suffix and never an owner or activation claim. */
  context(cookie: string): SetupState {
    const context = this.setup(cookie);
    return { status: 'ready', awsAccount: context.accountId.slice(-ACCOUNT_SUFFIX), contextId: context.nonce };
  }

  /** Authenticated reads distinguish durable registration from an unlinked setup. */
  async status(cookie: string, userId: string): Promise<SetupState> {
    const context = this.setup(cookie);
    const linked = await this.deps.repository.registered(context, userId);
    return { ...this.context(cookie), status: linked ? 'pending' : 'ready' };
  }

  /** An explicit authenticated action persists ownership before reporting pending. */
  async connect(cookie: string, userId: string, contextId: string): Promise<SetupState> {
    if (this.setup(cookie).nonce !== contextId)
      throw new HttpError(Status.CONFLICT, 'Review this purchase again before connecting', { code: 'setup_changed' });
    const identity = this.identity(cookie);
    await this.deps.repository.connect(identity, userId, this.deps.now());
    return this.status(cookie, userId);
  }

  /** Revalidates expiry and current product on every read and mutation. */
  private identity(cookie: string) {
    const { accountId, licenseArn, productCode } = this.setup(cookie);
    return { accountId, licenseArn, productCode };
  }

  /** The sealed nonce binds page consent to this exact handoff, never another tab's replacement. */
  private setup(cookie: string) {
    return openContext(cookie, this.deps.config().productCode, this.deps.now());
  }
}
