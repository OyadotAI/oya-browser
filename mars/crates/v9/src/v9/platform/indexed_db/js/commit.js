
  /**
   * Committing. A read/write transaction sends the coordinator only what it touched: clears,
   * then each touched key's final record (or its deletion), then moved key generators. An
   * upgrade sends the whole database image, since it may have renamed or dropped stores. Names
   * and keys travel as their JSON text, so lone surrogates survive UTF-8 storage. The coordinator
   * applies a commit atomically with the page's publication; a database past DATABASE_BYTES
   * aborts with QuotaExceededError instead.
   */

  /** The wire form of a name or key: its JSON text. */
  function wireName(text) {
    return jsonStringify(text);
  }

  /** A record's serialized value, cached until the record is replaced. */
  function wireValue(record) {
    if (record.wire === null) {
      record.wire = serializeValue(record.value);
    }
    return record.wire;
  }

  /** How metadata records an exhausted key generator, which JSON cannot write as Infinity. */
  const EXHAUSTED = -1;

  /** A store's metadata as JSON text: key path, generator and index definitions. */
  function storeMeta(store) {
    const indexes = [...store.indexes.values()].map((index) => [index.name, index.keyPath, index.unique, index.multiEntry]);
    const current = Number.isFinite(store.current) ? store.current : EXHAUSTED;
    return jsonStringify({ keyPath: store.keyPath, autoIncrement: store.autoIncrement, current, indexes });
  }

  /** The operations one read/write transaction commits. */
  function changeOperations(transaction) {
    const operations = [];
    for (const store of transaction.cleared) {
      if (!store.deleted) {
        operations.push({ op: 'clear', store: wireName(store.name) });
      }
    }
    for (const [store, keys] of transaction.dirty) {
      if (!store.deleted) {
        keys.forEach((key) => operations.push(recordOperation(store, key)));
      }
    }
    for (const store of transaction.meta) {
      if (!store.deleted) {
        operations.push({ op: 'meta', store: wireName(store.name), meta: storeMeta(store) });
      }
    }
    return operations;
  }

  /** A put of a touched key's final record, or its deletion. */
  function recordOperation(store, key) {
    const record = recordFor(store, key);
    const base = { store: wireName(store.name), key: wireName(key) };
    return record ? { op: 'put', ...base, value: wireValue(record) } : { op: 'delete', ...base };
  }

  /** The whole-database operation an upgrade commits. */
  function imageOperation(db) {
    const stores = {};
    for (const store of db.stores.values()) {
      const records = {};
      for (const record of store.records) {
        records[wireName(record.key)] = wireValue(record);
      }
      stores[wireName(store.name)] = { meta: storeMeta(store), records };
    }
    return { op: 'replace', version: db.version, stores };
  }

  /** The bytes a database's stored keys and values occupy. */
  function databaseBytes(db) {
    let bytes = 0;
    for (const store of db.stores.values()) {
      for (const record of store.records) {
        bytes += record.key.length + wireValue(record).length;
      }
    }
    return bytes;
  }

  /** "Commit a transaction": persist, then fire complete; an upgrade's open request continues afterwards. */
  function commitTransaction(transaction) {
    transaction.state = 'committing';
    const operations = transaction.mode === 'versionchange' ? [imageOperation(transaction.db)]
      : transaction.mode === 'readonly' ? [] : changeOperations(transaction);
    if (operations.length > 0 && !persist(transaction, operations)) {
      return;
    }
    transaction.undo = [];
    transaction.state = 'finished';
    fireEvent(transaction.object, 'complete', false, false);
    finishTransaction(transaction);
  }

  /** Hand a commit to the coordinator; on quota or host failure abort the transaction and report false. */
  function persist(transaction, operations) {
    const db = transaction.db;
    const bytes = databaseBytes(db);
    if (bytes > DATABASE_BYTES) {
      abortTransaction(transaction, domError('QuotaExceededError', 'The database is larger than its storage quota.'));
      return false;
    }
    if (!host.commit(jsonStringify({ database: wireName(db.name), ops: operations }))) {
      abortTransaction(transaction, domError('UnknownError', 'The database could not be saved.'));
      return false;
    }
    db.bytes = bytes;
    return true;
  }
