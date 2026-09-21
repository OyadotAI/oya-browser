/**
 * One backend the pool can route to, with its live load, failure cooldown and
 * latency.
 */
import { metrics } from '../../platform/metrics.ts';
import {
  BACKOFF_FACTOR,
  COOLDOWN_BASE_MS,
  COOLDOWN_MAX_MS,
  DEFAULT_MAX_CONCURRENT,
  DEFAULT_PRIORITY,
  LATENCY_KEEP,
  LATENCY_NEW,
} from './constants.ts';

/** `|| fallback` would turn a deliberate 0 into the default. */
const numOr = (v, fallback) => (Number.isFinite(Number(v)) ? Number(v) : fallback);

/** One backend the pool can route to, with its live load, failure cooldown and latency. */
export class Provider {
  /** Sessions it is serving now. */
  active = 0;
  /** Until when it is skipped after a failure. */
  cooldownUntil = 0;
  /** Off means never picked. */
  declare enabled: any;
  /** Consecutive failures, driving the backoff. */
  failures = 0;
  /** Moving average (EWMA) of connect time; null until measured. */
  latencyMs: any = null;
  /** Sessions it may serve at once. */
  declare maxConcurrent: any;
  /** Name, unique per owner. */
  declare name: any;
  /** null for shared host infrastructure (OYA_PROVIDERS), else the registering key's fingerprint. */
  declare owner: any;
  /** Lower wins under the priority strategy. */
  declare priority: any;
  /** Failures over its lifetime. */
  totalFailures = 0;
  /** Sessions acquired over its lifetime. */
  totalSessions = 0;
  /** 'cdp' (wsUrl) or a hosted vendor. */
  declare type: any;
  /** Share of sessions under the weighted strategy. */
  declare weight: any;
  /** CDP WebSocket URL for a 'cdp' provider. */
  declare wsUrl: any;

  /** Builds a provider from a validated config, with fresh counters. */
  constructor(cfg) {
    this.name = cfg.name;
    // null = shared infrastructure declared by the host (OYA_PROVIDERS).
    // Otherwise the fingerprint of the API key that registered it.
    this.owner = cfg.owner ?? null;
    this.type = cfg.type || 'cdp'; // 'cdp' (wsUrl) or a hosted vendor
    this.wsUrl = cfg.wsUrl || null;
    this.maxConcurrent = numOr(cfg.maxConcurrent, DEFAULT_MAX_CONCURRENT);
    this.weight = numOr(cfg.weight, 1);
    this.priority = numOr(cfg.priority, DEFAULT_PRIORITY); // lower wins
    this.enabled = cfg.enabled !== false;
  }

  /** Enabled and not cooling down. */
  get healthy() {
    return this.enabled && Date.now() >= this.cooldownUntil;
  }
  /** Below its concurrency limit. */
  get hasCapacity() {
    return this.active < this.maxConcurrent;
  }
  /** Healthy with room for another session. */
  get available() {
    return this.healthy && this.hasCapacity;
  }

  /** A connect worked: clear the backoff and fold the time into the latency average. */
  succeed(latencyMs) {
    this.failures = 0;
    this.cooldownUntil = 0;
    if (Number.isFinite(latencyMs)) {
      this.latencyMs = this.latencyMs == null ? latencyMs : this.latencyMs * LATENCY_KEEP + latencyMs * LATENCY_NEW;
    }
  }

  /** A connect failed: cool down for 5s, doubling per consecutive failure up to 5 minutes. */
  fail() {
    this.failures += 1;
    this.totalFailures += 1;
    // Exponential, capped. A flapping provider backs off without being
    // permanently written off.
    const backoff = Math.min(COOLDOWN_MAX_MS, COOLDOWN_BASE_MS * BACKOFF_FACTOR ** (this.failures - 1));
    this.cooldownUntil = Date.now() + backoff;
    metrics.providerFailures.inc({ provider: this.name });
  }

  /** The provider as the API reports it, with derived health and remaining cooldown. */
  toJSON() {
    return { ...this.identity(), ...this.settings(), ...this.health(), ...this.lifetime() };
  }

  /** What and whose it is. */
  private identity() {
    return { name: this.name, type: this.type, enabled: this.enabled, owner: this.owner, shared: this.owner === null };
  }

  /** Its load and routing settings. */
  private settings() {
    return { active: this.active, maxConcurrent: this.maxConcurrent, weight: this.weight, priority: this.priority };
  }

  /** Whether it can take a session, its latency and any remaining cooldown. */
  private health() {
    return {
      healthy: this.healthy,
      available: this.available,
      latencyMs: this.latencyMs == null ? null : Math.round(this.latencyMs),
      cooldownMsRemaining: Math.max(0, this.cooldownUntil - Date.now()),
    };
  }

  /** Lifetime counters. */
  private lifetime() {
    return { totalSessions: this.totalSessions, totalFailures: this.totalFailures };
  }
}
