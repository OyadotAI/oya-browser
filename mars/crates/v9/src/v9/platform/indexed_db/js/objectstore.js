
  /**
   * IDBObjectStore: a transaction's handle on one store. Every method checks, in the
   * specification's order, that the store still exists, that the transaction is active and (for
   * writes) writable, then queues its operation as a request.
   */

  const IDBObjectStore = makeInterface('IDBObjectStore', null);

  /** A new handle for `store` in `transaction`. */
  function newStoreHandle(transaction, store) {
    return plainInstance(IDBObjectStore, { brand: IDBObjectStore, transaction, store, indexHandles: new NativeMap(), keyPath: undefined });
  }

  /** The handle's state after the deleted-store and active-transaction checks. */
  function liveStore(handle) {
    const state = stateOf(handle, IDBObjectStore);
    if (state.store.deleted) {
      throw stateError('The object store has been deleted.');
    }
    requireActive(state.transaction);
    return state;
  }

  /** The handle's state after the checks a write adds. */
  function writableStore(handle) {
    const state = liveStore(handle);
    requireWritable(state.transaction);
    return state;
  }

  /** Clone a value with the transaction made inactive meanwhile, as "clone a value" requires. */
  function cloneInTransaction(transaction, value) {
    transaction.state = 'inactive';
    try {
      return cloneValue(value);
    } finally {
      transaction.state = 'active';
    }
  }

  /** The shared steps of put() and add(): validate, clone, extract or check the key, then queue "store a record". */
  function addOrPut(handle, value, key, noOverwrite) {
    const state = writableStore(handle);
    const store = state.store;
    if (key !== undefined && store.keyPath !== null) {
      throw dataError('The object store uses in-line keys and the key parameter was provided.');
    }
    if (key === undefined && store.keyPath === null && !store.autoIncrement) {
      throw dataError('The object store uses out-of-line keys and has no key generator and the key parameter was not provided.');
    }
    const explicitKey = key === undefined ? null : requireKey(key);
    const clone = cloneInTransaction(state.transaction, value);
    const resolved = store.keyPath === null ? explicitKey : inlineKey(store, clone);
    return queueRequest(state.transaction, handle, () => storeRecord(state.transaction, store, clone, resolved, noOverwrite)).object;
  }

  /** The key an in-line store finds in a value, or null when the generator will supply (and inject) one. */
  function inlineKey(store, clone) {
    const key = extractKey(clone, store.keyPath);
    if (key === null) {
      throw dataError('Evaluating the object store\'s key path yielded a value that is not a valid key.');
    }
    if (key !== NO_VALUE) {
      return key;
    }
    if (!store.autoIncrement) {
      throw dataError('Evaluating the object store\'s key path did not yield a value.');
    }
    if (!canInjectKey(clone, store.keyPath)) {
      throw dataError('A generated key could not be inserted into the value.');
    }
    return null;
  }

  /** "Store a record into an object store": generate or advance the key, check constraints, write. */
  function storeRecord(transaction, store, value, key, noOverwrite) {
    if (store.autoIncrement) {
      key = key === null ? generateKey(transaction, store, value) : advanceGenerator(transaction, store, key);
    }
    if (noOverwrite && recordFor(store, key)) {
      throw domError('ConstraintError', 'Key already exists in the object store.');
    }
    if (violatesUnique(store, key, value)) {
      throw domError('ConstraintError', 'Unable to add key to index: at least one key does not satisfy the uniqueness requirements.');
    }
    writeRecord(transaction, store, key, value);
    return keyValue(key);
  }

  /** "Generate a key" and inject it when the store has a key path. */
  function generateKey(transaction, store, value) {
    if (store.current > GENERATOR_LIMIT) {
      throw domError('ConstraintError', 'The key generator has reached its maximum value.');
    }
    const key = TAG_NUMBER + encodeNumber(store.current);
    setGenerator(transaction, store, store.current + 1);
    if (store.keyPath !== null) {
      injectKey(value, key, store.keyPath);
    }
    return key;
  }

  /** "Possibly update the key generator" for an explicit number key; returns the key. A generator past 2^53 is Infinity: exhausted. */
  function advanceGenerator(transaction, store, key) {
    if (key[0] !== TAG_NUMBER) {
      return key;
    }
    const value = mathFloor(mathMin(keyValue(key), GENERATOR_LIMIT));
    if (value >= store.current) {
      setGenerator(transaction, store, value >= GENERATOR_LIMIT ? Infinity : value + 1);
    }
    return key;
  }

  members(IDBObjectStore.prototype, {
    get name() { return stateOf(this, IDBObjectStore).store.name; },
    set name(value) { renameStoreHandle(this, String(value)); },
    get keyPath() {
      const state = stateOf(this, IDBObjectStore);
      if (state.keyPath === undefined) {
        state.keyPath = keyPathValue(state.store.keyPath);
      }
      return state.keyPath;
    },
    get indexNames() { return stringList(sortedNames(stateOf(this, IDBObjectStore).store.indexes.keys())); },
    get transaction() { return stateOf(this, IDBObjectStore).transaction.object; },
    get autoIncrement() { return stateOf(this, IDBObjectStore).store.autoIncrement; },
    put(value, key = undefined) {
      requireArgument(arguments, 1, 'put', 'IDBObjectStore');
      return addOrPut(this, value, key, false);
    },
    add(value, key = undefined) {
      requireArgument(arguments, 1, 'add', 'IDBObjectStore');
      return addOrPut(this, value, key, true);
    },
    delete(query) {
      requireArgument(arguments, 1, 'delete', 'IDBObjectStore');
      const state = writableStore(this);
      const range = toRange(query, true);
      return queueRequest(state.transaction, this, () => deleteRange(state.transaction, state.store, range)).object;
    },
    clear() {
      const state = writableStore(this);
      return queueRequest(state.transaction, this, () => clearStore(state.transaction, state.store)).object;
    },
    get(query) {
      requireArgument(arguments, 1, 'get', 'IDBObjectStore');
      return storeRead(this, toRangeChecked(this, query, true), (item) => (item ? cloneValue(item.value) : undefined));
    },
    getKey(query) {
      requireArgument(arguments, 1, 'getKey', 'IDBObjectStore');
      return storeRead(this, toRangeChecked(this, query, true), (item) => (item ? keyValue(item.key) : undefined));
    },
    getAll(queryOrOptions = undefined, count = undefined) { return storeAll(this, queryOrOptions, count, 'getAll', ALL_RESULTS.values); },
    getAllKeys(queryOrOptions = undefined, count = undefined) { return storeAll(this, queryOrOptions, count, 'getAllKeys', ALL_RESULTS.keys); },
    getAllRecords(options = undefined) { return storeAll(this, options === undefined ? {} : options, undefined, 'getAllRecords', ALL_RESULTS.records); },
    count(query = undefined) {
      const state = liveStore(this);
      const range = toRange(query, false);
      return queueRequest(state.transaction, this, () => countInRange(state.store, range)).object;
    },
    openCursor(query = undefined, direction = undefined) { return openStoreCursor(this, query, direction, false, 'openCursor'); },
    openKeyCursor(query = undefined, direction = undefined) { return openStoreCursor(this, query, direction, true, 'openKeyCursor'); },
    createIndex(name, keyPath, options = undefined) {
      requireArgument(arguments, 2, 'createIndex', 'IDBObjectStore');
      return createIndexOn(this, String(name), keyPath, options);
    },
    deleteIndex(name) {
      requireArgument(arguments, 1, 'deleteIndex', 'IDBObjectStore');
      deleteIndexOn(this, String(name));
    },
    index(name) {
      requireArgument(arguments, 1, 'index', 'IDBObjectStore');
      return indexHandleFor(this, String(name));
    },
  });

  /** Check the store and transaction before converting the query, as the specification orders it. */
  function toRangeChecked(handle, query, nullDisallowed) {
    liveStore(handle);
    return toRange(query, nullDisallowed);
  }

  /** Queue a read of the first record in a range. */
  function storeRead(handle, range, answer) {
    const state = liveStore(handle);
    return queueRequest(state.transaction, handle, () => answer(firstInRange(state.store, range))).object;
  }

  /** Queue a getAll-style read. */
  function storeAll(handle, queryOrOptions, count, operation, answer) {
    const state = liveStore(handle);
    const query = getAllArguments(queryOrOptions, count, operation, 'IDBObjectStore');
    return queueRequest(state.transaction, handle, () => answer(collect(state.store, query.range, query.count, query.direction))).object;
  }
