/**
 * Test doubles for the personas module: an in-memory repository that stores
 * what a real one would (JSON, uncapped as null), and the collaborators a
 * PersonaService is built from, each recording what it was asked.
 */
import { mock } from 'node:test';
import {
  PersonaService,
  type PersonaDeps,
  type PersonaRepository,
  type Persona,
} from '../../../src/modules/personas/index.ts';
import { shape } from '../../../src/modules/personas/model.ts';

/**
 * A repository holding each persona as the JSON a row would be (uncapped as
 * null), keyed by id, so a round trip is real. Saving upserts and never
 * deletes; only remove() does, as with the storage-backed one.
 */
export class MemoryPersonaRepository implements PersonaRepository {
  /** Stored JSON by persona id. */
  rows = new Map<string, string>();
  /** How many times saveAll ran. */
  saves = 0;
  /** When set, every call rejects with it. */
  failWith: Error | null = null;

  /** Every stored persona, normalised the way the storage repository does. */
  async loadAll(): Promise<Persona[]> {
    if (this.failWith) throw this.failWith;
    return [...this.rows.values()].map((json) => shape(JSON.parse(json)));
  }

  /** Upserts each persona as JSON, with Infinity written as null. */
  async saveAll(personas: Persona[]) {
    if (this.failWith) throw this.failWith;
    this.saves += 1;
    for (const p of personas)
      this.rows.set(
        p.id,
        JSON.stringify({ ...p, maxConcurrent: Number.isFinite(p.maxConcurrent) ? p.maxConcurrent : null }),
      );
  }

  /** Deletes the given ids. */
  async remove(ids: string[]) {
    if (this.failWith) throw this.failWith;
    for (const id of ids) this.rows.delete(id);
  }
}

/** Collaborators for a PersonaService: owner is `owner:<key>`, and each fake records its calls. */
export function personaDeps(overrides: Partial<PersonaDeps> = {}) {
  const deps = {
    repository: new MemoryPersonaRepository(),
    ownerOf: (key: string) => `owner:${key}`,
    proxies: { assigned: mock.fn((_id: string): any => null), residential: mock.fn((_p: Persona): any => null) },
    mfa: {
      describe: mock.fn(() => ({ configured: false })),
      list: mock.fn(() => []),
      clearAll: mock.fn(async () => 0),
    },
    credentials: { list: mock.fn(() => []), clearAll: mock.fn(async () => 0) },
    logins: { summary: mock.fn(() => null), clear: mock.fn() },
    metrics: { personaCapped: { inc: mock.fn() }, personasActive: { set: mock.fn() } },
    ...overrides,
  };
  return deps;
}

/** A service over fresh fakes, returned with them. */
export function personaService(overrides: Partial<PersonaDeps> = {}) {
  const deps = personaDeps(overrides);
  return { service: new PersonaService(deps), deps };
}
