/**
 * The tables the server keeps, and each column's type. Every driver reads this
 * to write a value and to turn what it stored back into the same JavaScript
 * value: a number for an int, an object for json, an ISO string for a time.
 * SQLite creates its tables from it; Postgres gets them from server/migrations.
 */

/** How a column's value is stored and read back. */
export type ColumnType = 'text' | 'int' | 'bool' | 'json' | 'time' | 'serial';

/** One table: its columns and the columns that identify a row. */
export type Table = {
  /** Column name to type. */
  columns: Record<string, ColumnType>;
  /** The primary key, which an upsert matches on. */
  key: string[];
};

/** Every usage counter a key accumulates per hour. */
export const USAGE_FIELDS = [
  'commands',
  'command_errors',
  'chat_requests',
  'chat_input_tokens',
  'chat_output_tokens',
  'browser_seconds',
  'browsers_started',
  'cookie_pulls',
  'frames',
  'sandboxes_created',
  'rate_limited',
  'quota_denied',
  'bytes_out',
  'residential_proxy_bytes',
];

/** Each table by name. */
export const TABLES: Record<string, Table> = {
  personas: {
    key: ['id'],
    columns: {
      ...{ id: 'text', owner: 'text', name: 'text', seed: 'int', prefs: 'json', proxy: 'json', device: 'json' },
      ...{ max_concurrent: 'int', is_default: 'bool', created_at: 'time', last_used_at: 'time', updated_at: 'time' },
    },
  },
  api_keys: {
    key: ['key_hash'],
    columns: {
      ...{ key_hash: 'text', key_prefix: 'text', project: 'text', user_id: 'text', label: 'text' },
      ...{ created_at: 'time', last_used_at: 'time', agent_email: 'text' },
    },
  },
  profiles: {
    key: ['id'],
    columns: { id: 'text', email: 'text', display_name: 'text', role: 'text', created_at: 'time' },
  },
  key_settings: { key: ['owner', 'key'], columns: { owner: 'text', key: 'text', value: 'text', updated_at: 'time' } },
  settings: { key: ['key'], columns: { key: 'text', value: 'text', updated_at: 'time' } },
  usage: {
    key: ['api_key', 'hour'],
    columns: {
      api_key: 'text',
      hour: 'time',
      ...Object.fromEntries(USAGE_FIELDS.map((f) => [f, 'int' as const])),
      updated_at: 'time',
    },
  },
  // Sealed records by id: a persona's site credentials, its MFA factors and its
  // login state (cookies and localStorage). Values are sealed before they get here.
  persona_credentials: { key: ['id'], columns: { id: 'text', value: 'text', updated_at: 'time' } },
  mfa_factors: { key: ['id'], columns: { id: 'text', value: 'text', updated_at: 'time' } },
  persona_logins: { key: ['id'], columns: { id: 'text', value: 'text', updated_at: 'time' } },
  // A project's routines, sealed; version guards every write (see modules/routines/repository.ts).
  routines: {
    key: ['owner', 'id'],
    columns: { owner: 'text', id: 'text', value: 'text', version: 'int', updated_at: 'time' },
  },
  audit_log: {
    key: ['id'],
    columns: {
      ...{ id: 'serial', ts: 'time', action: 'text', actor: 'text', actor_user: 'text', target_type: 'text' },
      ...{ target_id: 'text', outcome: 'text', ip: 'text', user_agent: 'text', meta: 'json' },
      ...{ chain: 'text', seq: 'int', prev_hash: 'text', hash: 'text' },
    },
  },
};

/** A table's column types, or none for a table outside the schema (legacy tables read once). */
export const columnsOf = (table: string): Record<string, ColumnType> =>
  Object.hasOwn(TABLES, table) ? TABLES[table].columns : {};

/** A stored value as JavaScript: numbers for ints, objects for json, booleans, ISO strings for times. */
export function fromStored(type: ColumnType | undefined, value: unknown) {
  if (value === null || value === undefined) return null;
  if (type === 'int' || type === 'serial') return Number(value);
  if (type === 'bool') return Boolean(value);
  if (type === 'json') return typeof value === 'string' ? JSON.parse(value) : value;
  if (type === 'time') return value instanceof Date ? value.toISOString() : String(value);
  return value;
}

/** A row read from any driver, each column turned back into its JavaScript value. */
export function rowFromStored(table: string, row: Record<string, unknown>) {
  const types = columnsOf(table);
  return Object.fromEntries(Object.entries(row).map(([col, value]) => [col, fromStored(types[col], value)]));
}
