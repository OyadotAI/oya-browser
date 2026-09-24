/**
 * A table of sealed records by id, for modules that keep a map in memory and
 * write each change through: credentials, MFA factors, login state. The values
 * arrive sealed; this only stores them.
 */
import { getConnection } from './factory.ts';

/** One record table. */
export class RecordTable {
  /** The table's name in the schema. */
  declare readonly table: string;
  constructor(table: string) {
    this.table = table;
  }

  /** Every record, as id -> sealed value. */
  async load() {
    const rows = await getConnection().select(this.table);
    return new Map<string, string>(rows.map((r) => [r.id, r.value]));
  }

  /** Stores records, replacing any with the same id. */
  async put(entries: [string, string][]) {
    const updated_at = new Date().toISOString();
    const rows = entries.map(([id, value]) => ({ id, value, updated_at }));
    if (rows.length) await getConnection().upsert(this.table, rows, { update: true });
  }

  /** Removes records by id. */
  async remove(ids: string[]) {
    for (const id of ids) await getConnection().delete(this.table, { id });
  }
}
