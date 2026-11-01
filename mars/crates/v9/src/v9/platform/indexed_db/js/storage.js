
  /**
   * The in-memory database model. A store keeps its records in an array sorted by encoded key;
   * an index keeps entries sorted by (index key, primary key), each pointing at its record.
   * Lookups are binary searches. Every mutation goes through a transaction, which journals an
   * undo step (so an abort restores the exact previous state) and marks the keys it touched (so
   * a commit sends only those records to the coordinator).
   * ponytail: inserts splice a sorted array (O(n) moves per write); switch to a B-tree when
   * stores of hundreds of thousands of records matter.
   */

  /** A database: its version, stores by name, open connections and its transactions in creation order. */
  function newDatabase(name, version) {
    return { name, version, stores: new NativeMap(), connections: new NativeSet(), transactions: [], bytes: 0 };
  }

  /** An object store. `current` is the key generator's next number. */
  function newStore(name, keyPath, autoIncrement) {
    return { name, keyPath, autoIncrement, current: 1, records: [], indexes: new NativeMap(), deleted: false };
  }

  /** An index over a store. */
  function newIndex(store, name, keyPath, unique, multiEntry) {
    return { store, name, keyPath, unique, multiEntry, entries: [], deleted: false };
  }

  /** The first position in a sorted array where `after(item)` holds; `after` must be false then true along the array. */
  function firstWhere(array, after) {
    let low = 0;
    let high = array.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if (after(array[middle])) {
        high = middle;
      } else {
        low = middle + 1;
      }
    }
    return low;
  }

  /** The position of the record with `key`, or -1. */
  function recordPosition(store, key) {
    const position = firstWhere(store.records, (record) => record.key >= key);
    return position < store.records.length && store.records[position].key === key ? position : -1;
  }

  /** The record with `key`, or undefined. */
  function recordFor(store, key) {
    const position = recordPosition(store, key);
    return position < 0 ? undefined : store.records[position];
  }

  /** The [start, end) positions of a store's records inside a range. */
  function recordSpan(store, range) {
    return spanOf(store.records, range, (record) => record.key);
  }

  /** The [start, end) positions of sorted items whose `keyOf` lies inside a range. */
  function spanOf(items, range, keyOf) {
    const start = range.lower === null ? 0
      : firstWhere(items, (item) => (range.lowerOpen ? keyOf(item) > range.lower : keyOf(item) >= range.lower));
    const end = range.upper === null ? items.length
      : firstWhere(items, (item) => (range.upperOpen ? keyOf(item) >= range.upper : keyOf(item) > range.upper));
    return [start, Math.max(start, end)];
  }

  /** The index keys a value contributes to an index: none, one, or (multiEntry) several. */
  function indexKeys(index, value) {
    const found = evaluateKeyPath(value, index.keyPath);
    if (found === NO_VALUE) {
      return [];
    }
    if (index.multiEntry && isArray(found)) {
      return multiEntryKeys(found);
    }
    const key = valueToKey(found);
    return key === null ? [] : [key];
  }

  /** The position for an index entry ordered by (index key, primary key). */
  function entryPosition(entries, indexKey, primaryKey) {
    return firstWhere(entries, (entry) => entry.ik > indexKey || (entry.ik === indexKey && entry.pk >= primaryKey));
  }

  /** Add a record's entries to every index of its store. */
  function addEntries(store, record) {
    for (const index of store.indexes.values()) {
      for (const indexKey of indexKeys(index, record.value)) {
        const position = entryPosition(index.entries, indexKey, record.key);
        index.entries.splice(position, 0, { ik: indexKey, pk: record.key, record });
      }
    }
  }

  /** Remove a record's entries from every index of its store. */
  function removeEntries(store, record) {
    for (const index of store.indexes.values()) {
      for (const indexKey of indexKeys(index, record.value)) {
        const position = entryPosition(index.entries, indexKey, record.key);
        if (position < index.entries.length && index.entries[position].record === record) {
          index.entries.splice(position, 1);
        }
      }
    }
  }

  /** Put `record` where its key belongs, replacing any record with the same key, without journaling. */
  function placeRecord(store, key, record) {
    const position = firstWhere(store.records, (item) => item.key >= key);
    const existing = position < store.records.length && store.records[position].key === key;
    if (existing) {
      removeEntries(store, store.records[position]);
      store.records.splice(position, 1);
    }
    if (record) {
      store.records.splice(position, 0, record);
      addEntries(store, record);
    }
  }

  /** Whether storing `value` under `key` would break a unique index (the record it replaces does not count). */
  function violatesUnique(store, key, value) {
    for (const index of store.indexes.values()) {
      if (index.unique && indexKeys(index, value).some((indexKey) => takenByOther(index, indexKey, key))) {
        return true;
      }
    }
    return false;
  }

  /** Whether an index key already belongs to a record other than `key`. */
  function takenByOther(index, indexKey, key) {
    const position = firstWhere(index.entries, (entry) => entry.ik >= indexKey);
    const entries = index.entries;
    return (position < entries.length && entries[position].ik === indexKey && entries[position].pk !== key)
      || (position + 1 < entries.length && entries[position + 1].ik === indexKey && entries[position + 1].pk !== key);
  }

  /** Fill a new index from its store's records; returns false when a unique index finds a duplicate. */
  function populateIndex(index) {
    const entries = [];
    for (const record of index.store.records) {
      for (const indexKey of indexKeys(index, record.value)) {
        entries.push({ ik: indexKey, pk: record.key, record });
      }
    }
    entries.sort((a, b) => (a.ik < b.ik ? -1 : a.ik > b.ik ? 1 : a.pk < b.pk ? -1 : a.pk > b.pk ? 1 : 0));
    index.entries = entries;
    return !index.unique || entries.every((entry, at) => at === 0 || entries[at - 1].ik !== entry.ik);
  }
