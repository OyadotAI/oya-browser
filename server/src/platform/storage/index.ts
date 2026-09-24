/**
 * Storage: the facade the rest of the server imports. getConnection() hands
 * out the one configured driver (Postgres, SQLite or JSON files); modules keep
 * their repositories against the Connection contract and never touch a
 * database client or the filesystem for their data themselves.
 */
export { getConnection, closeConnection, createConnection } from './factory.ts';
export { storageConfig, STORAGE_KINDS, type StorageConfig } from './config.ts';
export { pgRemote, closePgPool } from './postgres.ts';
export { USAGE_FIELDS } from './schema.ts';
export { importLegacyFile } from './legacy.ts';
export { RecordTable } from './records.ts';
export type { Connection, StorageKind, Row, Where, SelectOptions, ControlRemote } from './connection.ts';
