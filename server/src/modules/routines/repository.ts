/**
 * Where a project's routines are kept: one row per routine in the routines
 * table, sealed under the project, with a version that every write checks and
 * bumps. Reads go to the database each time, so every replica sees the same
 * routines, and a write that lost a race to another writer says so.
 */
import { getConnection } from '../../platform/storage/index.ts';
import { sealText, openText } from '../../platform/secrets.ts';
import type { Routine } from './rules.ts';

/** The table's name. */
const TABLE = 'routines';

/** A routine as read, with the version a write must match. */
export interface Stored {
  /** The routine. */
  routine: Routine;
  /** Its row's version. */
  version: number;
}

/** The sealing scope, so one project's routines cannot be opened as another's. */
const scopeOf = (owner: string) => `routines:${owner}`;

/** A row as a stored routine; one that no longer unseals reads as absent. */
function fromRow(owner: string, row): Stored | null {
  try {
    return { routine: openText(scopeOf(owner), row.value) as Routine, version: Number(row.version) };
  } catch {
    return null;
  }
}

/** Every routine of `owner`. */
export async function all(owner: string): Promise<Stored[]> {
  const rows = await getConnection().select(TABLE, { owner });
  return rows.map((row) => fromRow(owner, row)).filter(Boolean) as Stored[];
}

/** One routine of `owner`, or null. */
export async function one(owner: string, id: string): Promise<Stored | null> {
  const [row] = await getConnection().select(TABLE, { owner, id });
  return row ? fromRow(owner, row) : null;
}

/** Adds a routine at version 1; an id already there is left alone. */
export async function insert(owner: string, routine: Routine): Promise<void> {
  const row = { owner, id: routine.id, value: sealText(scopeOf(owner), routine), version: 1 };
  await getConnection().upsert(TABLE, [{ ...row, updated_at: new Date().toISOString() }]);
}

/** Replaces a routine only if it is still at `version`; false when another write got there first. */
export async function swap(owner: string, routine: Routine, version: number): Promise<boolean> {
  const set = { value: sealText(scopeOf(owner), routine), version: version + 1, updated_at: new Date().toISOString() };
  return (await getConnection().update(TABLE, { owner, id: routine.id, version }, set)) > 0;
}

/** Removes a routine. */
export async function remove(owner: string, id: string): Promise<boolean> {
  return (await getConnection().delete(TABLE, { owner, id })) > 0;
}
