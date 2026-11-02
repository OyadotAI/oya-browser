/**
 * Persona group storage: the persona_groups table, in whichever storage driver
 * is configured. Written through on every change and read on every start, with
 * no copy in memory: groups are small, and a copy on one replica would go stale
 * the moment another replica edited the group.
 */
import { getConnection, type Connection } from '../../platform/storage/index.ts';

/** A named set of one owner's personas. */
export interface PersonaGroup {
  /** The owning API key's fingerprint. */
  owner: string;
  /** What a start names; unique per owner. */
  name: string;
  /** The member persona ids, in the order they were given. */
  personaIds: string[];
  /** When it was made (ISO). */
  createdAt: string;
  /** When its members last changed (ISO). */
  updatedAt: string;
}

/** Where groups are kept. */
export interface GroupRepository {
  /** The owner's groups, by name. */
  list(owner: string): Promise<PersonaGroup[]>;
  /** One group, or null. */
  get(owner: string, name: string): Promise<PersonaGroup | null>;
  /** Stores a new group; false when the owner already had one of that name. */
  insert(group: PersonaGroup): Promise<boolean>;
  /** Replaces a group's members; false when there was no such group. */
  replace(owner: string, name: string, personaIds: string[]): Promise<boolean>;
  /** Deletes a group; false when there was none. */
  remove(owner: string, name: string): Promise<boolean>;
  /** Takes a persona out of every group of its owner. */
  dropMember(owner: string, personaId: string): Promise<void>;
}

/** The table. */
const TABLE = 'persona_groups';

/** A stored time as ISO: Postgres answers a Date, SQLite and files the text that was written. */
const iso = (value: unknown) => new Date(value as string).toISOString();

/** A group from a row; a member list that is not a list reads as empty. */
const fromRow = (row: any): PersonaGroup => ({
  owner: row.owner,
  name: row.name,
  personaIds: Array.isArray(row.persona_ids) ? row.persona_ids.map(String) : [],
  createdAt: iso(row.created_at),
  updatedAt: iso(row.updated_at),
});

/** A row for a group. */
const toRow = (g: PersonaGroup) => ({
  owner: g.owner,
  name: g.name,
  persona_ids: g.personaIds,
  created_at: g.createdAt,
  updated_at: g.updatedAt,
});

/** Whether two member lists are the same, in order. */
const sameMembers = (a: string[], b: string[]) => a.length === b.length && a.every((id, i) => id === b[i]);

/** Groups in the persona_groups table. */
export class GroupTable implements GroupRepository {
  /** Storage, by default the configured connection. */
  declare private readonly db: () => Connection;
  constructor(db: () => Connection = getConnection) {
    this.db = db;
  }

  async list(owner: string) {
    const rows = (await this.db().select(TABLE, { owner })).map(fromRow);
    return rows.sort((a, b) => a.name.localeCompare(b.name));
  }

  async get(owner: string, name: string) {
    const [row] = await this.db().select(TABLE, { owner, name });
    return row ? fromRow(row) : null;
  }

  /**
   * Refuses a name already taken, then inserts-or-ignores and reads back: a
   * stored row made at another time or with other members means a create racing
   * this one had the name first, so the two cannot both answer as made.
   */
  async insert(group: PersonaGroup) {
    if (await this.get(group.owner, group.name)) return false;
    await this.db().upsert(TABLE, [toRow(group)]);
    const stored = await this.get(group.owner, group.name);
    return stored?.createdAt === group.createdAt && sameMembers(stored.personaIds, group.personaIds);
  }

  async replace(owner: string, name: string, personaIds: string[]) {
    const set = { persona_ids: personaIds, updated_at: new Date().toISOString() };
    return (await this.db().update(TABLE, { owner, name }, set)) > 0;
  }

  async remove(owner: string, name: string) {
    return (await this.db().delete(TABLE, { owner, name })) > 0;
  }

  async dropMember(owner: string, personaId: string) {
    for (const g of await this.list(owner))
      if (g.personaIds.includes(personaId))
        await this.replace(
          owner,
          g.name,
          g.personaIds.filter((id) => id !== personaId),
        );
  }
}
