/**
 * Provider pool: which backend should this session go to.
 *
 * Providers are heterogeneous, a hosted vendor, a docker host, a plain Chrome,
 * so routing has to account for capacity, health and latency rather than
 * just taking turns. A saturated pool queues instead of failing, and a
 * provider that errors is put in cooldown rather than being retried forever.
 *
 * The provider itself, config validation, strategies, the wait queue and the
 * acquire loop each live in their own file; this one holds the pool.
 */
import { metrics } from '../../platform/metrics.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { validateProviderConfig } from './provider-config.ts';
import { Provider } from './provider.ts';
import { STRATEGIES, pickBy } from './strategies.ts';
import { SlotQueue } from './slot-queue.ts';
import { Acquisition } from './acquisition.ts';

export { STRATEGIES } from './strategies.ts';
export { validateProviderConfig } from './provider-config.ts';

/** Capacity, load and health summed over a list of reported providers. */
function totals(providers) {
  return {
    capacity: providers.reduce((n, p) => n + p.maxConcurrent, 0),
    active: providers.reduce((n, p) => n + p.active, 0),
    healthy: providers.filter((p) => p.healthy).length,
  };
}

/** Routes sessions across providers, queueing when all are busy and failing over when one errors. */
export class ProviderPool {
  /** Providers by owner-scoped key. */
  providers = new Map();
  /** Round-robin cursor. */
  rr = 0;
  /** Strategy each owner chose. */
  strategies = new Map();
  /** Host default strategy. */
  declare strategy: any;
  /** Callers queued for a free slot, oldest first. */
  private readonly queue = new SlotQueue();

  /** An empty pool with the host's default strategy. */
  constructor() {
    // Host default; each key may choose its own.
    this.strategy = STRATEGIES.includes(process.env.OYA_ROUTING_STRATEGY)
      ? process.env.OYA_ROUTING_STRATEGY
      : 'priority';
  }

  /** Config comes from OYA_PROVIDERS (JSON array) or register() at runtime. */
  loadFromEnv(env = process.env) {
    if (!env.OYA_PROVIDERS) return this;
    try {
      for (const cfg of JSON.parse(env.OYA_PROVIDERS)) this.register(cfg);
    } catch (e) {
      console.error('[routing] OYA_PROVIDERS is not valid JSON:', e.message);
    }
    return this;
  }

  /** The owner's chosen strategy, else the host default. */
  strategyFor(owner) {
    return this.strategies.get(owner) || this.strategy;
  }

  /** Set the owner's strategy; 400 for an unknown one. */
  setStrategy(owner, strategy) {
    if (!STRATEGIES.includes(strategy)) throw new HttpError(Status.BAD_REQUEST, 'Unknown strategy');
    this.strategies.set(owner, strategy);
  }

  /** Names are namespaced by owner so two keys can both have a "chrome". */
  key(owner, name) {
    return `${owner ?? '@shared'}::${name}`;
  }

  /** Add a provider, or update one in place keeping its live counters. Changing where an active provider connects is refused. */
  register(cfg) {
    cfg = validateProviderConfig(cfg);
    const id = this.key(cfg.owner ?? null, cfg.name);
    const existing = this.providers.get(id);
    if (existing) return this.update(existing, cfg);
    const p = new Provider(cfg);
    this.providers.set(id, p);
    return p;
  }

  /** Keep live counters across an edit; only settings change. */
  private update(existing, cfg) {
    if (existing.active && (existing.type !== cfg.type || existing.wsUrl !== cfg.wsUrl)) {
      throw new HttpError(Status.CONFLICT, 'End active sessions before changing this provider.');
    }
    Object.assign(existing, cfg);
    return existing;
  }

  /** Remove a provider; refused while it has active sessions. */
  remove(owner, name) {
    if (this.get(owner, name)?.active)
      throw new HttpError(Status.CONFLICT, 'End active sessions before removing this provider.');
    return this.providers.delete(this.key(owner, name));
  }
  /** The owner's own provider configs, without shared ones. */
  configs(owner) {
    return this.visible(owner)
      .filter((p) => p.owner === owner)
      .map((p) => validateProviderConfig(p));
  }
  /** A provider by owner and name. */
  get(owner, name) {
    return this.providers.get(this.key(owner, name));
  }

  /** What this key can see: its own providers plus shared host infrastructure. */
  visible(owner) {
    return [...this.providers.values()].filter((p) => p.owner === null || p.owner === owner);
  }

  /** Visible providers as the API reports them. */
  list(owner) {
    return this.visible(owner).map((p) => p.toJSON());
  }

  /** Candidates that could take a session for this owner right now. */
  candidates(owner) {
    return this.visible(owner).filter((p) => p.available);
  }

  /** Choose one available provider by strategy, skipping those in `exclude`. Null when none is available. */
  pick(owner, strategy = this.strategyFor(owner), exclude = new Set()) {
    const pool = this.candidates(owner).filter((p) => !exclude.has(this.key(p.owner, p.name)));
    if (!pool.length) return null;
    return pickBy(strategy, pool, this);
  }

  /**
   * Take a slot, trying each healthy provider in turn.
   * @param connect  async (provider) => session, failure marks that provider
   *                 down and the next one is tried.
   */
  async acquire(options: any = {}) {
    return new Acquisition(this, options).run();
  }

  /** Free a provider's slot and hand it to the longest waiter that can use it. */
  release(provider) {
    provider.active = Math.max(0, provider.active - 1);
    const waiter = this.queue.take(provider);
    if (!waiter) return;
    waiter.resolve(
      provider.available && !waiter.exclude.has(this.key(provider.owner, provider.name))
        ? provider
        : this.pick(waiter.owner, waiter.strategy, waiter.exclude),
    );
  }

  /** Queue until a slot frees up; null on timeout. */
  waitForSlot(timeoutMs, owner, strategy, exclude) {
    if (timeoutMs <= 0) return Promise.resolve(null);
    metrics.routingQueued.inc({});
    return this.queue.wait(timeoutMs, owner, strategy, exclude);
  }

  /** Callers waiting for a slot. */
  get queueDepth() {
    return this.queue.depth;
  }

  /** The owner's view of the pool: strategy, queue depth, capacity, load and providers. */
  stats(owner = null) {
    const providers = this.list(owner);
    return { strategy: this.strategyFor(owner), queueDepth: this.queueDepth, ...totals(providers), providers };
  }
}

/** The process-wide pool, seeded from OYA_PROVIDERS. */
export const pool = new ProviderPool().loadFromEnv();
