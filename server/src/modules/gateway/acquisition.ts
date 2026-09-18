/**
 * One acquire(): take a slot, trying each healthy provider in turn. A
 * provider that fails is put in cooldown and the next one is tried; when all
 * are busy the caller queues.
 */
import { control } from '../control/service.ts';
import { metrics } from '../../platform/metrics.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { CAPACITY_RETRY_MS, DEFAULT_ATTEMPTS, DEFAULT_QUEUE_MS } from './constants.ts';

/** hold() found no control-plane capacity yet: pick again. */
const CAPACITY_RETRY = Symbol('capacity retry');

/** acquire()'s options, with the queue time and attempt count defaulted. */
function withDefaults({ owner = null, strategy, connect, queueMs = DEFAULT_QUEUE_MS, attempts = DEFAULT_ATTEMPTS }) {
  return { owner, strategy, connect, queueMs, attempts };
}

/** The pool as an acquisition sees it. */
export interface AcquirePool {
  /** Chooses an available provider. */
  pick(owner, strategy, exclude): any;
  /** Queues until a slot frees up; null on timeout. */
  waitForSlot(timeoutMs, owner, strategy, exclude): Promise<any>;
  /** Frees a provider's slot. */
  release(provider): void;
  /** A provider's owner-scoped key. */
  key(owner, name): string;
}

/** One attempt to get a connected browser session from the pool. */
export class Acquisition {
  /** The pool providers come from. */
  declare private readonly pool: AcquirePool;
  /** Whose providers may be used. */
  declare private readonly owner: any;
  /** The strategy asked for; the owner's when undefined. */
  declare private readonly strategy: any;
  /** async (provider) => session; failure marks that provider down. */
  declare private readonly connect: (provider) => Promise<any>;
  /** How long to queue for a free slot. */
  declare private readonly queueMs: number;
  /** Providers tried before giving up. */
  declare private readonly attempts: number;
  /** When waiting for control-plane capacity stops. */
  declare private readonly deadline: number;
  /** Providers that failed during this acquisition. */
  private readonly excluded = new Set();
  /** Connect attempts made so far. */
  private attempt = 0;

  /** Starts the clock on an acquisition. */
  constructor(pool: AcquirePool, options: any = {}) {
    this.pool = pool;
    Object.assign(this, withDefaults(options));
    this.deadline = Date.now() + this.queueMs;
  }

  /** Tries providers until one connects; throws 503 when none is available and 502 when all failed. */
  async run() {
    while (this.attempt < this.attempts) {
      const provider = await this.nextProvider();
      const holdId = await this.hold(provider);
      if (holdId === CAPACITY_RETRY) continue;
      const result = await this.tryConnect(provider, holdId);
      if (result) return result;
    }
    throw new HttpError(Status.UNAVAILABLE, 'No browser provider available');
  }

  /** An available provider, else the first to free up; 503 if none does in time. */
  private async nextProvider() {
    const provider = this.pool.pick(this.owner, this.strategy, this.excluded);
    if (provider) return provider;
    // Everything is busy or cooling down: wait for a release rather than
    // failing a request that would succeed a second later.
    const waited = await this.pool.waitForSlot(this.queueMs, this.owner, this.strategy, this.excluded);
    if (waited) return waited;
    metrics.routingRejected.inc({ reason: 'saturated' });
    throw new HttpError(Status.UNAVAILABLE, 'No browser provider available');
  }

  /** Reserves the provider's slot in the control plane; CAPACITY_RETRY after a short pause when it is full. */
  private async hold(provider) {
    try {
      return await control().holdProvider(provider.owner, provider.name, provider.maxConcurrent);
    } catch (e) {
      if (e.code !== 'provider_capacity') throw e;
      if (Date.now() >= this.deadline) throw new HttpError(Status.UNAVAILABLE, 'Provider capacity exhausted');
      await new Promise((r) => setTimeout(r, CAPACITY_RETRY_MS));
      return CAPACITY_RETRY;
    }
  }

  /** Connects through the provider; undefined after a failure that leaves attempts to spare. */
  private async tryConnect(provider, holdId) {
    provider.active += 1;
    const started = Date.now();
    try {
      return this.connected(provider, holdId, await this.dial(provider), started);
    } catch (err) {
      await this.failed(provider, holdId, err);
    }
  }

  /** Calls the caller's connect function, unbound as it was given. */
  private dial(provider) {
    const connect = this.connect;
    return connect(provider);
  }

  /** Records the success and hands back the session with its one-shot release. */
  private connected(provider, holdId, session, started) {
    provider.succeed(Date.now() - started);
    provider.totalSessions += 1;
    metrics.routingAcquired.inc({ provider: provider.name });
    return { provider, session, holdId, release: this.releaser(provider, holdId) };
  }

  /** Frees the slot and the control-plane hold, once however often it is called. */
  private releaser(provider, holdId) {
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.pool.release(provider);
      return control().releaseProvider(holdId);
    };
  }

  /** Puts the provider in cooldown and fails over; 502 once every attempt has failed. */
  private async failed(provider, holdId, err) {
    this.attempt += 1;
    await this.retire(provider, holdId);
    console.error(`[routing] ${provider.name} failed (${err.message}); failing over`);
    if (this.attempt === this.attempts) {
      metrics.routingRejected.inc({ reason: 'all_failed' });
      throw new HttpError(Status.BAD_GATEWAY, `All providers failed. Last error: ${err.message}`);
    }
  }

  /** Gives back the failed provider's slot and excludes it from this acquisition. */
  private async retire(provider, holdId) {
    await control().releaseProvider(holdId);
    provider.active -= 1;
    provider.fail();
    this.excluded.add(this.pool.key(provider.owner, provider.name));
  }
}
