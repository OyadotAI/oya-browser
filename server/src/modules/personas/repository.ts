/**
 * Persona storage: the personas table, in whichever storage driver is
 * configured. Personas are the durable half of an identity; losing them would
 * orphan the cookie jars keyed by them, so a personas.json left from before
 * storage drivers is taken in once, seeds and all, before the first read.
 */
import { getConnection, importLegacyFile, type Connection } from '../../platform/storage/index.ts';
import { shape, type Persona } from './model.ts';

/** Where personas are kept. */
export interface PersonaRepository {
  /** Every stored persona. An empty store is an empty list, not an error. */
  loadAll(): Promise<Persona[]>;
  /** Upsert all of them. */
  saveAll(personas: Persona[]): Promise<void>;
  /** Remove these, which were deleted. Never inferred from what saveAll lacks: another replica may hold personas this one has not loaded. */
  remove(ids: string[]): Promise<void>;
}

/** Infinity does not survive JSON; storage writes an uncapped persona as null. */
const cap = (p: Persona) => (Number.isFinite(p.maxConcurrent) ? p.maxConcurrent : null);

/** A persona from a `personas` table row. */
const fromRow = (row: any) =>
  shape({
    ...row,
    maxConcurrent: row.max_concurrent,
    isDefault: row.is_default,
    createdAt: row.created_at,
    lastUsedAt: row.last_used_at,
  });

/** A `personas` table row for a persona. */
const toRow = (p: Persona) => ({
  id: p.id,
  owner: p.owner,
  name: p.name,
  seed: p.seed,
  prefs: p.prefs,
  proxy: p.proxy,
  device: p.device,
  ...usageRow(p),
});

/** The row's cap, default flag and timestamps. */
const usageRow = (p: Persona) => ({
  max_concurrent: cap(p),
  is_default: p.isDefault,
  created_at: p.createdAt,
  last_used_at: p.lastUsedAt,
  updated_at: new Date().toISOString(),
});

/** Personas in the `personas` table. */
export class PersonaTable implements PersonaRepository {
  /** The legacy personas.json, imported on the first read. */
  declare readonly legacyFile: string;
  /** Storage, by default the configured connection. */
  declare private readonly db: () => Connection;
  constructor(legacyFile: string, db: () => Connection = getConnection) {
    this.legacyFile = legacyFile;
    this.db = db;
  }

  async loadAll() {
    // Upsert without replacing: a persona already in storage wins over the stale file.
    await importLegacyFile(this.legacyFile, (personas) => this.db().upsert('personas', personas.map(shape).map(toRow)));
    return (await this.db().select('personas')).map(fromRow);
  }

  async saveAll(personas: Persona[]) {
    if (personas.length) await this.db().upsert('personas', personas.map(toRow), { update: true });
  }

  async remove(ids: string[]) {
    for (const id of ids) await this.db().delete('personas', { id });
  }
}
