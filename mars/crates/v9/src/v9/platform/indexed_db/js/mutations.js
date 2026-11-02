
  /**
   * Journaled mutations. Each one changes the model, records how to undo it on the transaction,
   * and marks what the commit must send: touched record keys, cleared stores, stores whose key
   * generator moved. Schema changes only happen in upgrade transactions, which commit a whole
   * database image, so they only journal.
   */

  /** Record how to undo a change. */
  function journal(transaction, undo) {
    transaction.undo.push(undo);
  }

  /** Mark a record key the commit must send. */
  function touch(transaction, store, key) {
    let keys = call(mapGet, transaction.dirty, store);
    if (!keys) {
      keys = new NativeSet();
      transaction.dirty.set(store, keys);
    }
    keys.add(key);
  }

  /** Store `value` under `key`, replacing any record with that key. */
  function writeRecord(transaction, store, key, value) {
    const previous = recordFor(store, key) || null;
    placeRecord(store, key, { key, value, wire: null });
    journal(transaction, () => placeRecord(store, key, previous));
    touch(transaction, store, key);
  }

  /** Delete the records whose keys lie in a range. */
  function deleteRange(transaction, store, range) {
    const [start, end] = recordSpan(store, range);
    const removed = store.records.slice(start, end);
    for (const record of removed) {
      removeEntries(store, record);
      touch(transaction, store, record.key);
    }
    store.records.splice(start, end - start);
    journal(transaction, () => removed.forEach((record) => placeRecord(store, record.key, record)));
  }

  /** Delete every record of a store. */
  function clearStore(transaction, store) {
    const records = store.records;
    const entries = [...store.indexes.values()].map((index) => [index, index.entries]);
    store.records = [];
    entries.forEach(([index]) => { index.entries = []; });
    journal(transaction, () => {
      store.records = records;
      entries.forEach(([index, saved]) => { index.entries = saved; });
    });
    transaction.cleared.add(store);
    transaction.dirty.delete(store);
  }

  /** Move a store's key generator. */
  function setGenerator(transaction, store, current) {
    const previous = store.current;
    store.current = current;
    journal(transaction, () => { store.current = previous; });
    transaction.meta.add(store);
  }

  /** Add a store to the database. */
  function addStore(transaction, store) {
    const db = transaction.db;
    db.stores.set(store.name, store);
    journal(transaction, () => {
      db.stores.delete(store.name);
      store.deleted = true;
    });
  }

  /** Remove a store from the database; its records stay with it so an abort can bring it back. */
  function removeStore(transaction, store) {
    const db = transaction.db;
    db.stores.delete(store.name);
    store.deleted = true;
    journal(transaction, () => {
      store.deleted = false;
      db.stores.set(store.name, store);
    });
  }

  /** Rename a store. */
  function renameStore(transaction, store, name) {
    const db = transaction.db;
    const previous = store.name;
    db.stores.delete(previous);
    store.name = name;
    db.stores.set(name, store);
    journal(transaction, () => {
      db.stores.delete(name);
      store.name = previous;
      db.stores.set(previous, store);
    });
  }

  /** Add an index to a store. */
  function addIndex(transaction, index) {
    const store = index.store;
    store.indexes.set(index.name, index);
    journal(transaction, () => {
      store.indexes.delete(index.name);
      index.deleted = true;
    });
  }

  /** Remove an index from a store. */
  function removeIndex(transaction, index) {
    const store = index.store;
    store.indexes.delete(index.name);
    index.deleted = true;
    journal(transaction, () => {
      index.deleted = false;
      store.indexes.set(index.name, index);
    });
  }

  /** Rename an index. */
  function renameIndex(transaction, index, name) {
    const store = index.store;
    const previous = index.name;
    store.indexes.delete(previous);
    index.name = name;
    store.indexes.set(name, index);
    journal(transaction, () => {
      store.indexes.delete(name);
      index.name = previous;
      store.indexes.set(previous, index);
    });
  }

  /** Set the database version (an upgrade), restoring it on abort. */
  function setVersion(transaction, version) {
    const db = transaction.db;
    const previous = db.version;
    db.version = version;
    journal(transaction, () => { db.version = previous; });
  }
