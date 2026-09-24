/**
 * The one contract every storage driver meets. Modules keep their repositories
 * against this and never learn which driver is underneath: Postgres, SQLite or
 * JSON files, chosen once by configuration (config.ts, factory.ts).
 */

/** Which driver stores the data. */
export type StorageKind = 'postgres' | 'sqlite' | 'file';

/** A row: column name to value. */
export type Row = Record<string, any>;

/**
 * A filter, every entry of which must hold. A plain value is equality, null is
 * "is null", `{ gte }` is a lower bound and `{ notIn }` excludes a list.
 */
export type Where = Record<string, unknown>;

/** How a select orders and cuts its rows. */
export type SelectOptions = {
  /** Column and direction to sort by. */
  order?: [string, 'asc' | 'desc'];
  /** At most this many rows. */
  limit?: number;
};

/** What one control function call answers: its value, or why it failed. */
export type RpcResult = {
  /** The function's single value. */
  data?: unknown;
  /** The failure, with the database's own message. */
  error?: {
    /** The database's error text, which RemoteBackend reads to tell a conflict from an outage. */
    message: string;
  };
};

/** The control plane's `{ rpc }` client, which RemoteBackend speaks to. */
export type ControlRemote = {
  /** Calls one control function with named arguments. */
  rpc: (name: string, args?: Record<string, unknown>) => Promise<RpcResult>;
};

/** How an upsert treats a row whose key is already stored. */
export type UpsertOptions = {
  /** Replace the stored row; otherwise it is left as it is. */
  update?: boolean;
};

/** A storage driver. */
export interface Connection {
  /** Which driver this is, for the startup log. */
  readonly kind: StorageKind;
  /** Rows of `table` matching `where`. */
  select(table: string, where?: Where, options?: SelectOptions): Promise<Row[]>;
  /** Inserts `rows`; with `update`, a row whose key exists is replaced, otherwise it is left alone. */
  upsert(table: string, rows: Row[], options?: UpsertOptions): Promise<void>;
  /** Sets `set` on the rows matching `where`; answers how many changed. */
  update(table: string, where: Where, set: Row): Promise<number>;
  /** Deletes the rows matching `where`; answers how many went. */
  delete(table: string, where: Where): Promise<number>;
  /** Whether `table` exists here: legacy tables only some deployments ever had. */
  exists(table: string): Promise<boolean>;
  /** The control plane's remote client, or null to keep it in its own SQLite file. */
  controlRemote(): ControlRemote | null;
  /** Releases the driver's resources, for shutdown and tests. */
  close(): Promise<void>;
}
