/**
 * Persona storage. Personas are the durable half of an identity; losing them
 * would orphan the cookie jars keyed by them, so a database that cannot be
 * reached falls back to a local file rather than dropping writes.
 */
import { readFile, writeFile, mkdir } from 'fs/promises';
import { dirname } from 'path';
import type { SupabaseClient } from '@supabase/supabase-js';
import { shape, type Persona } from './model.ts';
import { JSON_INDENT } from './constants.ts';

/** Where personas are kept. */
export interface PersonaRepository {
  /** Every stored persona. An empty store is an empty list, not an error. */
  loadAll(): Promise<Persona[]>;
  /** Upsert all of them. */
  saveAll(personas: Persona[]): Promise<void>;
}

/** Infinity does not survive JSON; storage writes an uncapped persona as null. */
const cap = (p: Persona) => (Number.isFinite(p.maxConcurrent) ? p.maxConcurrent : null);

/** The personas file's contents. */
const toJson = (personas: Persona[]) =>
  JSON.stringify(
    personas.map((p) => ({ ...p, maxConcurrent: cap(p) })),
    null,
    JSON_INDENT,
  );

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

/** Personas as a JSON file, written owner-only (0600). */
export class FilePersonaRepository implements PersonaRepository {
  /** Path of the JSON file. */
  declare readonly path: string;
  constructor(path: string) {
    this.path = path;
  }

  async loadAll() {
    try {
      return (JSON.parse(await readFile(this.path, 'utf8')) as unknown[]).map(shape);
    } catch (e) {
      if ((e as NodeJS.ErrnoException).code === 'ENOENT') return [];
      throw e;
    }
  }

  async saveAll(personas: Persona[]) {
    await mkdir(dirname(this.path), { recursive: true });
    await writeFile(this.path, toJson(personas), { mode: 0o600 });
  }
}

/** Personas in the Supabase `personas` table. */
export class SupabasePersonaRepository implements PersonaRepository {
  /** Supabase client for the table. */
  declare readonly db: SupabaseClient<any, any, any>;
  constructor(db: SupabaseClient<any, any, any>) {
    this.db = db;
  }

  async loadAll() {
    const { data, error } = await this.db.from('personas').select('*');
    if (error) throw new Error(error.message);
    return (data || []).map(fromRow);
  }

  async saveAll(personas: Persona[]) {
    const { error } = await this.db.from('personas').upsert(personas.map(toRow), { onConflict: 'id' });
    if (error) throw new Error(error.message);
  }
}

/** The database, and the file whenever the database fails. */
export class FallbackPersonaRepository implements PersonaRepository {
  /** The database repository. */
  declare readonly primary: PersonaRepository;
  /** The file used when the database fails. */
  declare readonly fallback: FilePersonaRepository;
  /** Set after the first failed write, so the fallback warning is logged once. */
  declare private warned: boolean;
  constructor(primary: PersonaRepository, fallback: FilePersonaRepository) {
    this.primary = primary;
    this.fallback = fallback;
    this.warned = false;
  }

  async loadAll() {
    try {
      return await this.primary.loadAll();
    } catch (e) {
      // A missing table or an unreachable database must not lose personas.
      this.warn('read', e);
      return this.fallback.loadAll();
    }
  }

  async saveAll(personas: Persona[]) {
    try {
      await this.primary.saveAll(personas);
    } catch (e) {
      this.warnOnce(e);
      await this.fallback.saveAll(personas);
    }
  }

  /** Logs the first failed write only, so a down database does not flood the log. */
  private warnOnce(e: unknown) {
    if (this.warned) return;
    this.warn('write', e);
    this.warned = true;
  }

  /** Logs that a database `op` failed and the file is standing in. */
  private warn(op: 'read' | 'write', e: unknown) {
    console.error(`[personas] database ${op} failed (${(e as Error).message}), falling back to ${this.fallback.path}`);
  }
}
