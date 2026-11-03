/** Seller-authenticated AWS identity resolution with bounded work and safe public errors. */
import {
  MarketplaceMeteringClient,
  ResolveCustomerCommand,
  type ResolveCustomerResult,
} from '@aws-sdk/client-marketplace-metering';
import { AWS_ATTEMPTS, AWS_CONCURRENCY, AWS_TIMEOUT_MS } from './constants.ts';
import { invalidSetup, type BuyerIdentity } from './model.ts';
import { unavailable } from './config.ts';

/** The AWS seam allows hermetic token-exchange tests without credentials or network. */
export type ResolveToken = (token: string, region: string) => Promise<BuyerIdentity>;

/** The SDK seam keeps tests independent of credentials and network. */
export interface ResolverClient {
  /** Sends one identity resolution using an externally owned deadline. */
  send(
    command: ResolveCustomerCommand,
    options: { /** Cancels both network work and response adoption. */ abortSignal: AbortSignal },
  ): Promise<ResolveCustomerResult>;
}

/** Bounded SDK transport with credential refresh delegated to the standard chain. */
function createClient(region: string): ResolverClient {
  return new MarketplaceMeteringClient({
    region,
    maxAttempts: AWS_ATTEMPTS,
    requestHandler: { connectionTimeout: AWS_TIMEOUT_MS, requestTimeout: AWS_TIMEOUT_MS },
  });
}

/** A deadline must reject even if credential resolution ignores the abort signal. */
function aborted(signal: AbortSignal): Promise<never> {
  return new Promise((_resolve, reject) =>
    signal.addEventListener('abort', () => reject(unavailable()), { once: true }),
  );
}

/** The operation owns its timer and always releases it; late responses cannot be adopted. */
async function timedExchange(send: (signal: AbortSignal) => Promise<ResolveCustomerResult>, release: () => void) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), AWS_TIMEOUT_MS);
  const work = exchangeWork(send, controller.signal, release);
  try {
    return await Promise.race([work, aborted(controller.signal)]);
  } finally {
    clearTimeout(timeout);
  }
}

/** Capacity is released only when the underlying SDK settles, even after a caller timeout. */
function exchangeWork(
  send: (signal: AbortSignal) => Promise<ResolveCustomerResult>,
  signal: AbortSignal,
  release: () => void,
) {
  return Promise.resolve()
    .then(() => send(signal))
    .finally(release);
}

/** AWS's caller-input errors are distinct from outages, never echoed verbatim. */
function resolutionError(error: unknown) {
  const name = error instanceof Error ? error.name : '';
  return ['ExpiredTokenException', 'InvalidTokenException', 'InvalidParameterException'].includes(name)
    ? invalidSetup()
    : unavailable();
}

/** Reuses each region's SDK client; the standard credential chain owns credential refresh. */
export class AwsBuyerResolver {
  /** Per-process admission cap bounds concurrent external exchanges. */
  private active = 0;
  /** Clients hold connection pools, with a bounded configured region set. */
  private clients = new Map<string, ResolverClient>();
  /** Client construction can be replaced by hermetic test doubles. */
  private factory: (region: string) => ResolverClient;

  /** Owns its bounded SDK clients without reaching into the composition root. */
  constructor(factory = createClient) {
    this.factory = factory;
  }

  /** Resolves a token immediately, releasing capacity on success or failure. */
  async resolve(token: string, region: string): Promise<BuyerIdentity> {
    if (this.active >= AWS_CONCURRENCY) throw unavailable();
    this.active++;
    try {
      return await this.exchange(token, region);
    } catch (error) {
      throw resolutionError(error);
    }
  }

  /** Converts AWS's response identifiers without falling back to CustomerIdentifier. */
  private async exchange(token: string, region: string) {
    const command = new ResolveCustomerCommand({ RegistrationToken: token });
    const result = await timedExchange(
      (signal) => this.client(region).send(command, { abortSignal: signal }),
      () => {
        this.active--;
      },
    );
    return { accountId: result.CustomerAWSAccountId, licenseArn: result.LicenseArn, productCode: result.ProductCode };
  }

  /** Bounds network waits as well as the operation's abort signal. */
  private client(region: string) {
    if (!this.clients.has(region)) this.clients.set(region, this.factory(region));
    return this.clients.get(region);
  }
}
