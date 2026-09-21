/**
 * In-memory stand-ins for the draft and run stores (DraftStore's interface),
 * so workspace tests never touch the disk or the keychain.
 */

/** A store holding copies of whatever it is given; `failSave` makes every save throw. */
class MemoryStore {
  /** Starts with `records` (id → record). */
  constructor(records = {}, { failSave = false } = {}) {
    /** Stored records by id. */
    this.records = new Map(Object.entries(records));
    /** Make save() throw, as a locked keychain does. */
    this.failSave = failSave;
    /** Ids removed, in order. */
    this.removed = [];
  }

  /** Summaries, newest first, as DraftStore lists them. */
  list() {
    const all = [...this.records.values()].map((r) => ({ id: r.id, name: r.name, updatedAt: r.updatedAt }));
    return all.sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0));
  }

  /** A copy of a record; a missing one throws. */
  load(id) {
    if (!this.records.has(id)) throw new Error(`No record ${id}`);
    return structuredClone(this.records.get(id));
  }

  /** Stores a copy. */
  save(record) {
    if (this.failSave) throw new Error('Secure local storage is unavailable.');
    this.records.set(record.id, structuredClone(record));
    return record;
  }

  /** A fixed size per record, in bytes. */
  size() {
    return 1024;
  }

  /** Deletes a record. */
  remove(id) {
    this.removed.push(id);
    this.records.delete(id);
  }
}

module.exports = { MemoryStore };
