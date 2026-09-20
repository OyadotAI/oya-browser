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

/** A repository holding personas as the JSON a file would, so a round trip is real. */
export class MemoryPersonaRepository implements PersonaRepository {
  /** The stored JSON, or null before the first save. */
  stored: string | null = null;
  /** How many times saveAll ran. */
  saves = 0;
  /** When set, both calls reject with it. */
  failWith: Error | null = null;

  /** Every stored persona, normalised the way the file repository does. */
  async loadAll(): Promise<Persona[]> {
    if (this.failWith) throw this.failWith;
    return this.stored ? (JSON.parse(this.stored) as unknown[]).map(shape) : [];
  }

  /** Stores the personas as JSON, with Infinity written as null. */
  async saveAll(personas: Persona[]) {
    if (this.failWith) throw this.failWith;
    this.saves += 1;
    this.stored = JSON.stringify(
      personas.map((p) => ({ ...p, maxConcurrent: Number.isFinite(p.maxConcurrent) ? p.maxConcurrent : null })),
    );
  }
}

/** Collaborators for a PersonaService: owner is `owner:<key>`, and each fake records its calls. */
export function personaDeps(overrides: Partial<PersonaDeps> = {}) {
  const deps = {
    repository: new MemoryPersonaRepository(),
    ownerOf: (key: string) => `owner:${key}`,
    proxies: { assigned: mock.fn((_id: string): any => null), residential: mock.fn((_p: Persona): any => null) },
    mfa: { describe: mock.fn(() => ({ configured: false })), list: mock.fn(() => []), clearAll: mock.fn() },
    credentials: { list: mock.fn(() => []), clearAll: mock.fn() },
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
