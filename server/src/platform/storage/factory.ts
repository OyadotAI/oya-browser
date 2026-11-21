/**
 * Builds the configured driver, once. getConnection() is the only way the rest
 * of the server reaches storage, so the choice between Postgres, SQLite and
 * files is made here and nowhere else.
 */
import { dataPath } from '../paths.ts';
import { storageConfig, type StorageConfig } from './config.ts';
import { PostgresConnection } from './postgres.ts';
import { SqliteConnection } from './sqlite.ts';
import { FileConnection } from './file.ts';
import { SQLITE_FILE, FILE_DIR } from './constants.ts';
import type { Connection, StorageKind } from './connection.ts';

/** How each driver is built from the configuration. */
const DRIVERS: Record<StorageKind, (config: StorageConfig) => Connection> = {
  postgres: (config) => new PostgresConnection(config.databaseUrl),
  sqlite: () => new SqliteConnection(dataPath(SQLITE_FILE)),
  file: () => new FileConnection(dataPath(FILE_DIR)),
};

/** A new driver for `config`. */
export function createConnection(config: StorageConfig = storageConfig()): Connection {
  const connection = DRIVERS[config.kind](config);
  console.log(`[storage] ${connection.kind}`);
  return connection;
}

// A configuration that cannot work stops the process as it starts, not at the first write.
storageConfig();

/** The process-wide connection. */
let connection: Connection | null = null;

/** The configured storage driver, built on first use and shared by every caller. */
export function getConnection(): Connection {
  return (connection ||= createConnection());
}

/** Keel's own connection, when KEEL_DATABASE_URL names one. */
let keelConnection: Connection | null = null;

/**
 * Where Keel's usage tables live: Keel's own database when KEEL_DATABASE_URL is set, so one app's load or
 * outage never reaches another's; the main connection otherwise, as a self-hosted server keeps everything in one.
 */
export function getKeelConnection(config: StorageConfig = storageConfig()): Connection {
  if (!config.keelDatabaseUrl) return getConnection();
  return (keelConnection ||= new PostgresConnection(config.keelDatabaseUrl));
}

/** Closes and forgets both connections, for shutdown and for tests that change the configuration. */
export async function closeConnection() {
  const open = [connection, keelConnection];
  connection = null;
  keelConnection = null;
  await Promise.all(open.map((c) => c?.close()));
}
