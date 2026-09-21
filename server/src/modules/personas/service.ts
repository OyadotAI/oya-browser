/**
 * Personas: creating, resolving and running as them. See model.ts for what a
 * persona is and why its device never changes.
 */
import { createHash } from 'crypto';
import { getFingerprintForPersona, previewProfile, defaultPersonaSeed, newPersonaSeed } from './fingerprint.ts';
import { seedFromString } from './prng.ts';
import {
  shape,
  markChecked,
  cleanPrefs,
  DEFAULT_MAX_CONCURRENT,
  DEFAULT_PERSONA_MAX_CONCURRENT,
  type Persona,
  type PersonaPrefs,
  type PersonaProxy,
} from './model.ts';
import type { PersonaRepository } from './repository.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MAX_NAME_CHARS, MIRROR_ID_HEX_CHARS } from './constants.ts';
import { PersonaStore } from './store.ts';
import { PersonaSlots } from './slots.ts';
import { describePersona } from './view.ts';

/** What the service needs from the rest of the server. Wired in app/container.ts. */
export interface PersonaDeps {
  /** Where personas are loaded from and saved to. */
  repository: PersonaRepository;
  /** The owner fingerprint of an API key. */
  ownerOf: (apiKey: string) => string;
  /** The proxy a persona is assigned or pinned to, and the residential fallback. */
  proxies: { assigned(personaId: string): any; residential(persona: Persona): any };
  /** Second factors stored for a persona. */
  mfa: { describe(personaId: string): unknown; list(personaId: string): unknown; clearAll(personaId: string): void };
  /** Site credentials stored for a persona. */
  credentials: { list(personaId: string): unknown; clearAll(personaId: string): void };
  /** Login state kept for a persona. */
  logins: { summary(personaId: string): unknown; clear(personaId: string): void };
  /** Gauges and counters the service reports into. */
  metrics: {
    /** Counts starts refused by a persona's concurrency cap. */
    personaCapped: { inc(labels: object): void };
    /** Browsers currently running as any persona. */
    personasActive: { set(labels: object, value: number): void };
  };
}

/** What a caller may choose when creating a persona. */
export type CreatePersona = {
  /** Display name; defaults to the id. */
  name?: string;
  /** The persona's own proxy. */
  proxy?: PersonaProxy | null;
  /** Concurrency cap; anything but a positive number means the default. */
  maxConcurrent?: number;
  /** Device choices, fixed for the persona's life. */
  prefs?: PersonaPrefs | null;
  /** Whether prefs are validated under the current rule; false only for clones of older personas. */
  prefsChecked?: boolean;
};
/** The fields that may change after creation; the device never does. */
export type UpdatePersona = {
  /** New display name. */
  name?: string;
  /** New concurrency cap; null means no cap. */
  maxConcurrent?: number | null;
  /** New proxy, or null to drop it. */
  proxy?: PersonaProxy | null;
};
/** What a caller may choose when cloning a persona. */
type ClonePersona = {
  /** Name for the copy; defaults to the source's plus "(copy)". */
  name?: string;
};

/** One real browser profile to mirror into a persona. */
export type MirrorProfile = {
  /** The source browser, e.g. "chrome"; part of the persona's stable id. */
  source: string;
  /** The profile directory, e.g. "Default"; part of the persona's stable id. */
  profile: string;
  /** Display name for the persona; defaults to the profile directory. */
  name?: string;
  /** The captured real-device fingerprint this persona runs as. */
  device: any;
};

/** The stable id of the persona mirroring one profile: never collides across owners or profiles. */
const mirroredId = (owner: string, source: string, profile: string) =>
  'm-' + createHash('sha256').update(`${owner}:${source}:${profile}`).digest('hex').slice(0, MIRROR_ID_HEX_CHARS);

/** The id and owner a mirrored persona is created under. */
type MirroredOwner = Pick<Persona, 'id' | 'owner'>;

/** A persona record that runs as a captured real device; its seed is derived from its stable id. */
const mirroredPersona = (owner: MirroredOwner, name: string, device: any): Persona =>
  shape({
    ...owner,
    name: name.slice(0, MAX_NAME_CHARS),
    seed: seedFromString(owner.id),
    device,
    maxConcurrent: DEFAULT_MAX_CONCURRENT,
    createdAt: new Date().toISOString(),
  });

/** A positive number as given, or `fallback`. */
const positiveOr = (value: unknown, fallback: number) => (Number(value) > 0 ? Number(value) : fallback);

/** The cap an update asks for: null or Infinity lifts it, anything not positive means the default. */
function capFor(p: Persona, maxConcurrent: number | null) {
  // The default persona's cap is a deployment decision, not a per-key one.
  if (p.isDefault && maxConcurrent !== null) return positiveOr(maxConcurrent, DEFAULT_PERSONA_MAX_CONCURRENT);
  if (maxConcurrent === null || maxConcurrent === Infinity) return Infinity;
  return positiveOr(maxConcurrent, DEFAULT_MAX_CONCURRENT);
}

/** A copy's creation options: the source's device choices under the source's rule, its proxy and cap. */
const copyOf = (src: Persona, name?: string): CreatePersona => ({
  name: name || `${src.name} (copy)`,
  prefs: src.prefs,
  // Same kind of device as the source actually is, under the source's rule.
  prefsChecked: src.prefs?.checked === true,
  proxy: src.proxy,
  maxConcurrent: Number.isFinite(src.maxConcurrent) ? src.maxConcurrent : undefined,
});

/** Who a new persona is: its id, seed and owning key's fingerprint. */
type PersonaIds = Pick<Persona, 'id' | 'seed' | 'owner'>;

/** A key's default persona record, uncapped unless the deployment says otherwise. */
function defaultPersona(ids: PersonaIds) {
  return shape({
    ...ids,
    name: 'Default',
    isDefault: true,
    maxConcurrent: DEFAULT_PERSONA_MAX_CONCURRENT,
    createdAt: new Date().toISOString(),
  });
}

/** A new named persona record from what the caller chose. */
function newPersona(ids: PersonaIds, { name, proxy, maxConcurrent, prefs, prefsChecked = true }: CreatePersona) {
  return shape({
    ...ids,
    name: (name || ids.id).slice(0, MAX_NAME_CHARS),
    prefs: markChecked(cleanPrefs(prefs), prefsChecked),
    proxy: proxy || null,
    maxConcurrent: positiveOr(maxConcurrent, DEFAULT_MAX_CONCURRENT),
    createdAt: new Date().toISOString(),
  });
}

/** Least recently used first; never-used personas lead. */
const byLastUse = (a: Persona, b: Persona) => String(a.lastUsedAt || '').localeCompare(String(b.lastUsedAt || ''));

/** Owns every persona in memory, enforces per-persona concurrency, and saves changes on a timer. */
export class PersonaService {
  /** Collaborators wired in by the composition root. */
  declare private readonly deps: PersonaDeps;
  /** Every persona, saved on a timer. */
  declare private readonly store: PersonaStore;
  /** Browsers running as each persona. */
  declare private readonly slots: PersonaSlots;

  /** A service over `deps`; call restore() to load saved personas. */
  constructor(deps: PersonaDeps) {
    this.deps = deps;
    this.store = new PersonaStore(deps.repository);
    this.slots = new PersonaSlots(deps.metrics);
  }

  /** Public view: fingerprint included, seed and raw proxy credentials not. */
  describe(p: Persona) {
    return describePersona(p, this, this.deps);
  }

  /**
   * The key's default persona, created on first use. Its seed reproduces the
   * pre-persona fingerprint for that key exactly.
   */
  defaultFor(apiKey: string) {
    const { id, seed } = defaultPersonaSeed(apiKey);
    const existing = this.store.get(id);
    if (existing) return existing;
    const p = defaultPersona({ id, seed, owner: this.deps.ownerOf(apiKey) });
    this.store.put(p);
    return p;
  }

  /** A new persona for this key with a fresh seed; its prefs and seed never change after this. */
  create(apiKey: string, options: CreatePersona = {}) {
    const { id, seed } = newPersonaSeed();
    const p = newPersona({ id, seed, owner: this.deps.ownerOf(apiKey) }, options);
    this.store.put(p);
    return p;
  }

  /**
   * What a persona may change after creation: its label, its concurrency cap,
   * its proxy geo hint. Never seed or prefs, those are the device, and a
   * device that changes under an existing cookie jar is the tell this whole
   * model exists to avoid. Callers wanting a different device clone instead.
   */
  update(apiKey: string, id: string, { name, maxConcurrent, proxy }: UpdatePersona = {}) {
    const p = this.get(apiKey, id);
    if (!p) return null;
    if (name !== undefined) p.name = String(name || p.id).slice(0, MAX_NAME_CHARS);
    if (maxConcurrent !== undefined) p.maxConcurrent = capFor(p, maxConcurrent);
    if (proxy !== undefined) p.proxy = proxy && typeof proxy === 'object' ? proxy : null;
    this.store.touch();
    return p;
  }

  /** Same device choices, a fresh seed: a new machine of the same kind. */
  clone(apiKey: string, id: string, { name }: ClonePersona = {}) {
    const src = this.get(apiKey, id);
    return src ? this.create(apiKey, copyOf(src, name)) : null;
  }

  /** The fingerprint a persona created with these prefs would get. Persists nothing. */
  preview(prefs: unknown) {
    const { id, seed } = newPersonaSeed();
    return previewProfile({ id: `preview-${id}`, seed, prefs: markChecked(cleanPrefs(prefs), true) });
  }

  /** Every persona this key owns, the default included. */
  list(apiKey: string) {
    const owner = this.deps.ownerOf(apiKey);
    this.defaultFor(apiKey); // always present
    return this.store.all().filter((p) => p.owner === owner);
  }

  /** The persona, or null when it does not exist or belongs to another key. */
  get(apiKey: string, id: string) {
    const p = this.store.get(id);
    // Ownership is the whole boundary: a persona id must not be usable by
    // another key, or one customer drives another's logged-in sessions.
    return p && p.owner === this.deps.ownerOf(apiKey) ? p : null;
  }

  /** Deletes a persona and everything stored for it; the default and personas in use are refused. */
  remove(apiKey: string, id: string) {
    const p = this.get(apiKey, id);
    if (!p) return false;
    if (p.isDefault) throw new HttpError(Status.BAD_REQUEST, 'The default persona cannot be deleted');
    if (this.activeCount(id) > 0) throw new HttpError(Status.CONFLICT, 'Persona is in use');
    this.forget(id);
    return true;
  }

  /** Drops a persona with its login state, factors, credentials and slots. */
  private forget(id: string) {
    this.store.delete(id);
    this.deps.logins.clear(id);
    // clearAll, not clear: a persona may hold a factor and a credential per
    // portal, and leaving those behind would outlive the identity they belong to.
    this.deps.mfa.clearAll(id);
    this.deps.credentials.clearAll(id);
    this.slots.delete(id);
  }

  /** Resolve what a browser should run as. Unknown or unowned ids are refused. */
  resolve(apiKey: string, personaId?: string | null) {
    if (!personaId || personaId === 'default') return this.defaultFor(apiKey);
    if (personaId === 'auto') return this.leastUsed(apiKey);
    const p = this.get(apiKey, personaId);
    if (!p) throw new HttpError(Status.NOT_FOUND, `No such persona: ${personaId}`);
    return p;
  }

  /** The key's least recently used persona with a free slot; 429 when every one is full. */
  private leastUsed(apiKey: string) {
    const owned = this.list(apiKey)
      .filter((p) => this.activeCount(p.id) < (p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT))
      .sort(byLastUse);
    if (!owned.length) throw new HttpError(Status.TOO_MANY_REQUESTS, 'Every persona is at its concurrency cap');
    return owned[0];
  }

  /**
   * The device fingerprint a persona runs as: its captured real device when it
   * mirrors one, otherwise the profile its seed, prefs and proxy generate. The
   * id and proxy are always the persona's own, so a device captured on one
   * machine still carries this persona's id and proxy.
   */
  fingerprintFor(persona: Pick<Persona, 'id' | 'seed' | 'prefs' | 'proxy' | 'device'>) {
    if (persona.device)
      return { ...persona.device, id: persona.id, proxy: persona.proxy || persona.device.proxy || null };
    return getFingerprintForPersona({ id: persona.id, seed: persona.seed, prefs: persona.prefs, proxy: persona.proxy });
  }

  /**
   * The persona that mirrors one real browser profile. Its id is derived from
   * the owner, source browser and profile, so re-importing the same profile
   * finds the same persona instead of making a second one. The device is set
   * once, on creation, and left immutable like the seed.
   */
  mirror(apiKey: string, { source, profile, name, device }: MirrorProfile) {
    const owner = this.deps.ownerOf(apiKey);
    const id = mirroredId(owner, source, profile);
    return this.store.get(id) || this.putMirrored({ id, owner }, name || profile, device);
  }

  /** Stores a new mirrored persona and returns it. */
  private putMirrored(owner: MirroredOwner, name: string, device: any) {
    const p = mirroredPersona(owner, name, device);
    this.store.put(p);
    return p;
  }

  /**
   * Take a concurrency slot. Refused past the cap rather than silently allowed:
   * a single device running more sessions than a person plausibly could is a
   * signal no amount of fingerprint work hides.
   */
  acquire(persona: Persona, browserId: string) {
    this.slots.acquire(persona, browserId);
    persona.lastUsedAt = new Date().toISOString();
    this.store.touch();
    this.slots.report();
    return persona;
  }

  /** Frees the browser's concurrency slot. */
  release(persona: Persona | null | undefined, browserId: string) {
    this.slots.release(persona, browserId);
  }

  /** Browsers running as this persona now. */
  activeCount(id: string) {
    return this.slots.count(id);
  }

  // ── Persistence ──

  /** Loads saved personas at startup; a failure is logged, not thrown. */
  async restore() {
    await this.store.restore();
  }

  /** Write changes every 10s. The composition root starts this; tests may not. */
  startAutosave(everyMs?: number) {
    this.store.startAutosave(everyMs);
  }

  /** Stops autosave and writes out pending changes, for shutdown. */
  async drain() {
    await this.store.drain();
  }

  /** Test hook. */
  reset() {
    this.store.clear();
    this.slots.clear();
  }
}
