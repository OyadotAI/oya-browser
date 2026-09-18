/**
 * One registered proxy: sealed credentials plus the health, cooldown and
 * assignment state the pool uses to pick one.
 */
import { randomBytes } from 'crypto';
import { metrics } from '../../platform/metrics.ts';
import { assignments } from './store.ts';
import { BACKOFF_FACTOR, COOLDOWN_MS, ID_BYTES, MAX_COOLDOWN_MS } from './constants.ts';

/** A fresh proxy id: "px-" plus random hex. */
export const newProxyId = () => 'px-' + randomBytes(ID_BYTES).toString('hex');

/** A registered proxy: sealed credentials plus the health, cooldown and assignment state used to pick one. */
export class Proxy {
  /** Until when a failed proxy is skipped; backs off exponentially with consecutive failures. */
  cooldownUntil = 0;
  /** Egress address seen by the last successful check. */
  declare exitIp: any;
  /** Consecutive failed checks. */
  failures = 0;
  /** Country or region code, e.g. "US", "US-CA", "DE". */
  declare geo: any;
  /** False after a failure until a check succeeds. */
  declare healthy: any;
  /** "px-" plus random hex. */
  declare id: any;
  /** "residential" or "datacenter". */
  declare kind: string;
  /** Display name; the id when none was given. */
  declare label: any;
  /** ISO time of the last successful check. */
  declare lastCheckedAt: any;
  /** How many personas may share this exit. */
  declare maxPersonas: number;
  /** Owning API key; null for shared host infrastructure. */
  declare owner: any;
  /** Encrypted { url, username, password }. */
  declare sealed: any;

  /** Builds a proxy from its stored or registered config. */
  constructor(cfg) {
    this.id = cfg.id || newProxyId();
    this.owner = cfg.owner ?? null; // null = shared host infrastructure
    this.label = cfg.label || this.id;
    this.kind = cfg.kind === 'datacenter' ? 'datacenter' : 'residential';
    this.geo = cfg.geo || null; // e.g. "US", "US-CA", "DE"
    this.maxPersonas = Number(cfg.maxPersonas) > 0 ? Number(cfg.maxPersonas) : 1;
    this.sealed = cfg.sealed; // encrypted { url, username, password }
    this.restoreHealth(cfg);
  }

  /** Health as last known, so a reloaded proxy keeps its exit IP and check time. */
  private restoreHealth(cfg) {
    this.healthy = cfg.healthy !== false;
    this.exitIp = cfg.exitIp || null;
    this.lastCheckedAt = cfg.lastCheckedAt || null;
  }

  /** Healthy and out of cooldown. */
  get available() {
    return this.healthy && Date.now() >= this.cooldownUntil;
  }
  /** Number of personas currently assigned to this proxy. */
  get assigned() {
    return [...assignments.values()].filter((id) => id === this.id).length;
  }

  /** Record a failure: mark unhealthy and back off before it is offered again. */
  fail() {
    this.failures += 1;
    this.cooldownUntil = Date.now() + Math.min(COOLDOWN_MS * BACKOFF_FACTOR ** (this.failures - 1), MAX_COOLDOWN_MS);
    this.healthy = false;
    metrics.proxyFailures.inc({ kind: this.kind });
  }

  /** Record a successful check and clear the failure state. */
  succeed(exitIp) {
    this.failures = 0;
    this.cooldownUntil = 0;
    this.healthy = true;
    if (exitIp) this.exitIp = exitIp;
    this.lastCheckedAt = new Date().toISOString();
  }

  /** Credentials never leave the server in a response. */
  toJSON() {
    return { ...this.identity(), ...this.health(), ...this.load() };
  }

  /** What the proxy is, for toJSON. */
  private identity() {
    return { id: this.id, label: this.label, kind: this.kind, geo: this.geo, shared: this.owner === null };
  }

  /** Whether it works and where it exits, for toJSON. */
  private health() {
    return {
      healthy: this.healthy,
      available: this.available,
      exitIp: this.exitIp,
      lastCheckedAt: this.lastCheckedAt,
    };
  }

  /** How full it is and how long it is still cooling down, for toJSON. */
  private load() {
    return {
      assigned: this.assigned,
      maxPersonas: this.maxPersonas,
      cooldownMsRemaining: Math.max(0, this.cooldownUntil - Date.now()),
    };
  }
}
