/**
 * Which storage driver runs, from configuration, read in this one place.
 *
 *   OYA_STORAGE   postgres | sqlite | file   (default: DEFAULT_STORAGE)
 *   DATABASE_URL  the Postgres connection string, required by postgres and
 *                 refused by the others, so a deployment that set it before
 *                 OYA_STORAGE existed cannot land on SQLite by accident
 *   KEEL_DATABASE_URL  Keel's own Postgres database, for Keel's usage tables;
 *                 unset, they stay in DATABASE_URL's (a self-hosted server)
 *
 * Supabase is sign-in only and keeps no data, so it needs postgres: its
 * accounts live beside the data it signs people in to.
 */
import { DEFAULT_STORAGE } from './constants.ts';
import type { StorageKind } from './connection.ts';

/** The drivers there are. */
export const STORAGE_KINDS: StorageKind[] = ['postgres', 'sqlite', 'file'];

/** The resolved storage configuration. */
export type StorageConfig = {
  /** Which driver. */
  kind: StorageKind;
  /** The Postgres connection string, for postgres. */
  databaseUrl?: string;
  /** Keel's own database, when it has one; Keel's tables live in the main one otherwise. */
  keelDatabaseUrl?: string;
};

/** The configured driver, refusing a configuration that would put data somewhere nobody meant. */
export function storageConfig(env: Record<string, string | undefined> = process.env): StorageConfig {
  const kind = (env.OYA_STORAGE || DEFAULT_STORAGE).trim().toLowerCase() as StorageKind;
  const problem = problemWith(kind, env);
  if (problem) throw new Error(problem);
  const keel = env.KEEL_DATABASE_URL ? { keelDatabaseUrl: env.KEEL_DATABASE_URL } : {};
  return { kind, databaseUrl: env.DATABASE_URL, ...keel };
}

/** Why `kind` cannot run with this environment, or null when it can. */
function problemWith(kind: StorageKind, env: Record<string, string | undefined>) {
  if (!STORAGE_KINDS.includes(kind))
    return `OYA_STORAGE must be one of ${STORAGE_KINDS.join(', ')}, not "${env.OYA_STORAGE}"`;
  if (kind === 'postgres' && !env.DATABASE_URL) return 'OYA_STORAGE=postgres needs DATABASE_URL';
  // A deployment from before OYA_STORAGE sets only DATABASE_URL; quietly using SQLite there would split its data.
  if (kind !== 'postgres' && env.DATABASE_URL)
    return `DATABASE_URL is set but OYA_STORAGE is ${kind}; set OYA_STORAGE=postgres`;
  if (env.SUPABASE_URL && kind !== 'postgres')
    return 'Supabase sign-in needs OYA_STORAGE=postgres, with DATABASE_URL set to its Postgres connection string';
  return keelProblem(kind, env);
}

/** Keel's database is Postgres beside a Postgres main one; beside SQLite or files its rows would split across two stores. */
const keelProblem = (kind: StorageKind, env: Record<string, string | undefined>) =>
  env.KEEL_DATABASE_URL && kind !== 'postgres' ? `KEEL_DATABASE_URL is set but OYA_STORAGE is ${kind}` : null;
