/**
 * Named values for storage: the configured default driver, where the local
 * drivers keep their files, and the modes those files are written with.
 */

/** The driver used when OYA_STORAGE is not set: one SQLite file, one replica. */
export const DEFAULT_STORAGE = 'sqlite';
/** The SQLite driver's database file, in the data directory. */
export const SQLITE_FILE = 'storage.sqlite';
/** The file driver's directory, one JSON file per table, in the data directory. */
export const FILE_DIR = 'storage';
/** Stored data is readable by the server's user only. */
export const PRIVATE_FILE_MODE = 0o600;
/** Its directories likewise. */
export const PRIVATE_DIR_MODE = 0o700;
/** Indentation of the file driver's JSON, so an operator can read it. */
export const FILE_INDENT = 2;
