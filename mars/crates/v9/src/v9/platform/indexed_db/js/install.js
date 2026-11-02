
  /**
   * Loading database images, the IDBFactory members, and installation. The coordinator answers a
   * load with JSON text: null, or { version, stores: { <name JSON>: { meta, records: { <key JSON>:
   * <value JSON> } } } }. Values are revived and indexes rebuilt when a database loads.
   */

  /** A database model from the coordinator's image, or null when the database does not exist. */
  function loadImage(name, image) {
    const parsed = jsonParse(image);
    if (parsed === null) {
      return null;
    }
    const db = newDatabase(name, parsed.version);
    for (const storeName of objectKeys(parsed.stores)) {
      const stored = parsed.stores[storeName];
      const store = loadStore(jsonParse(storeName), jsonParse(stored.meta), stored.records);
      db.stores.set(store.name, store);
    }
    return db;
  }

  /** A store model with its records sorted and its indexes rebuilt. */
  function loadStore(name, meta, records) {
    const store = newStore(name, meta.keyPath, meta.autoIncrement);
    store.current = meta.current === EXHAUSTED ? Infinity : meta.current;
    for (const wireKey of objectKeys(records)) {
      const wire = records[wireKey];
      store.records.push({ key: jsonParse(wireKey), value: deserializeValue(wire), wire });
    }
    store.records.sort((a, b) => compareKeys(a.key, b.key));
    for (const [indexName, keyPath, unique, multiEntry] of meta.indexes) {
      const index = newIndex(store, indexName, keyPath, unique, multiEntry);
      populateIndex(index);
      store.indexes.set(indexName, index);
    }
    return store;
  }

  /** Throw SecurityError where storage is unavailable: an opaque origin. */
  function requireOrigin() {
    if (ORIGIN === 'null') {
      throw domError('SecurityError', 'Access to the IndexedDB API is denied in this context.');
    }
  }

  /** open()'s version argument: [EnforceRange] unsigned long long, and never 0. */
  function versionArgument(value) {
    const number = Number(value);
    if (!Number.isFinite(number) || Math.trunc(number) < 0 || Math.trunc(number) > MAX_SAFE) {
      throw new NativeTypeError("Failed to execute 'open' on 'IDBFactory': Value is outside the 'unsigned long long' value range.");
    }
    if (Math.trunc(number) === 0) {
      throw new NativeTypeError("Failed to execute 'open' on 'IDBFactory': The version provided must not be 0.");
    }
    return Math.trunc(number);
  }

  members(IDBFactory.prototype, {
    open(name, version = undefined) {
      stateOf(this, IDBFactory);
      requireArgument(arguments, 1, 'open', 'IDBFactory');
      const text = String(name);
      const target = version === undefined ? undefined : versionArgument(version);
      requireOrigin();
      const request = newRequest(null, null, true);
      enqueue(text, () => withDatabase(text, (db, error) => openLoaded(request, text, target, db, error)));
      return request.object;
    },
    deleteDatabase(name) {
      stateOf(this, IDBFactory);
      requireArgument(arguments, 1, 'deleteDatabase', 'IDBFactory');
      const text = String(name);
      requireOrigin();
      const request = newRequest(null, null, true);
      enqueue(text, () => withDatabase(text, (db, error) => deleteLoaded(request, text, db, error)));
      return request.object;
    },
    databases() {
      stateOf(this, IDBFactory);
      return new NativePromise((resolve, reject) => {
        requireOrigin();
        if (!host.list((listing) => (typeof listing === 'string' ? resolve(databaseList(listing)) : reject(loadError())))) {
          reject(loadError());
        }
      });
    },
    cmp(first, second) {
      stateOf(this, IDBFactory);
      requireArgument(arguments, 2, 'cmp', 'IDBFactory');
      return compareKeys(requireKey(first), requireKey(second));
    },
  });

  /** databases()'s answer: the committed list, with databases this context changed since taking precedence. */
  function databaseList(listing) {
    const versions = new NativeMap();
    for (const [wire, version] of jsonParse(listing)) {
      versions.set(jsonParse(wire), version);
    }
    for (const [name, db] of registry) {
      if (db.version > 0) {
        versions.set(name, db.version);
      }
    }
    return [...versions].map(([name, version]) => ({ name, version }));
  }

  /** The one IDBFactory of this context. */
  const factory = plainInstance(IDBFactory, { brand: IDBFactory });

  /** Where `indexedDB` lives: beside localStorage (Window.prototype on a page), or on the global itself. */
  function attributeHolder() {
    for (let target = global; target; target = getPrototypeOf(target)) {
      if (getOwnPropertyDescriptor(target, 'localStorage')) {
        return target;
      }
    }
    return global;
  }

  /** Install a global interface binding, as WebIDL does: writable, configurable, not enumerable. */
  function defineGlobal(name, value) {
    defineProperty(global, name, { value, writable: true, enumerable: false, configurable: true });
  }

  const indexedDBGetter = { get indexedDB() { return factory; } };
  defineProperty(attributeHolder(), 'indexedDB', {
    get: getOwnPropertyDescriptor(indexedDBGetter, 'indexedDB').get, enumerable: true, configurable: true,
  });
  for (const [name, value] of [
    ['IDBFactory', IDBFactory], ['IDBDatabase', IDBDatabase], ['IDBTransaction', IDBTransaction],
    ['IDBObjectStore', IDBObjectStore], ['IDBIndex', IDBIndex], ['IDBCursor', IDBCursor],
    ['IDBCursorWithValue', IDBCursorWithValue], ['IDBKeyRange', IDBKeyRange], ['IDBRequest', IDBRequest],
    ['IDBOpenDBRequest', IDBOpenDBRequest], ['IDBVersionChangeEvent', IDBVersionChangeEvent],
    ['IDBRecord', IDBRecord], ['DOMStringList', DOMStringList],
  ]) {
    defineGlobal(name, value);
  }
  host.cleanup(cleanupTransactions);
})
