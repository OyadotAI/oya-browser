
  /**
   * Schema changes: creating, deleting and renaming stores and indexes. All of them need the
   * connection's upgrade transaction, active; each check throws in the specification's order.
   */

  /** Throw unless `transaction` is an active upgrade transaction. */
  function requireUpgrade(transaction) {
    if (!transaction || transaction.mode !== 'versionchange' || transaction.state === 'finished') {
      throw stateError('The database is not running a version change transaction.');
    }
    requireActive(transaction);
  }

  /** IDBDatabase.createObjectStore's steps. */
  function createStoreOn(connection, name, options) {
    const dictionary = options === undefined || options === null ? {} : options;
    const autoIncrement = Boolean(dictionary.autoIncrement);
    const keyPathInput = dictionary.keyPath;
    const transaction = connection.upgrade;
    requireUpgrade(transaction);
    const keyPath = keyPathInput === undefined || keyPathInput === null ? null : keyPathArgument(keyPathInput);
    if (keyPath !== null && !validKeyPath(keyPath)) {
      throw domError('SyntaxError', 'The keyPath option is not a valid key path.');
    }
    if (transaction.db.stores.has(name)) {
      throw domError('ConstraintError', 'An object store with the specified name already exists.');
    }
    if (autoIncrement && (keyPath === '' || isArray(keyPath))) {
      throw domError('InvalidAccessError', 'The autoIncrement option was set but the keyPath option was empty or an array.');
    }
    const store = newStore(name, keyPath, autoIncrement);
    addStore(transaction, store);
    return storeHandle(transaction, store);
  }

  /** IDBDatabase.deleteObjectStore's steps. */
  function deleteStoreOn(connection, name) {
    const transaction = connection.upgrade;
    requireUpgrade(transaction);
    const store = transaction.db.stores.get(name);
    if (!store) {
      throw domError('NotFoundError', 'The specified object store was not found.');
    }
    removeStore(transaction, store);
  }

  /** IDBObjectStore's name setter. */
  function renameStoreHandle(handle, name) {
    const state = stateOf(handle, IDBObjectStore);
    if (state.store.deleted) {
      throw stateError('The object store has been deleted.');
    }
    requireUpgrade(state.transaction);
    if (state.store.name === name) {
      return;
    }
    if (state.transaction.db.stores.has(name)) {
      throw domError('ConstraintError', 'An object store with the specified name already exists.');
    }
    renameStore(state.transaction, state.store, name);
  }

  /** IDBObjectStore.createIndex's steps; a unique index the records already break aborts the upgrade. */
  function createIndexOn(handle, name, keyPathInput, options) {
    const state = stateOf(handle, IDBObjectStore);
    requireUpgradeOnStore(state);
    if (state.store.indexes.has(name)) {
      throw domError('ConstraintError', 'An index with the specified name already exists.');
    }
    const { keyPath, unique, multiEntry } = indexArguments(keyPathInput, options);
    const index = newIndex(state.store, name, keyPath, unique, multiEntry);
    addIndex(state.transaction, index);
    if (!populateIndex(index)) {
      const transaction = state.transaction;
      queueTask(() => abortTransaction(transaction, domError('ConstraintError', 'Unable to add key to index: at least one key does not satisfy the uniqueness requirements.')));
    }
    return indexHandle(handle, index);
  }

  /** createIndex's key path and options, validated in the specification's order. */
  function indexArguments(keyPathInput, options) {
    const keyPath = keyPathArgument(keyPathInput);
    if (!validKeyPath(keyPath)) {
      throw domError('SyntaxError', 'The keyPath argument contains an invalid key path.');
    }
    const dictionary = options === undefined || options === null ? {} : options;
    const multiEntry = Boolean(dictionary.multiEntry);
    const unique = Boolean(dictionary.unique);
    if (multiEntry && isArray(keyPath)) {
      throw domError('InvalidAccessError', 'The keyPath argument was an array and the multiEntry option is true.');
    }
    return { keyPath, unique, multiEntry };
  }

  /** The upgrade checks a store's schema methods share: an upgrade transaction, the store alive, the transaction active. */
  function requireUpgradeOnStore(state) {
    const transaction = state.transaction;
    if (transaction.mode !== 'versionchange') {
      throw stateError('The database is not running a version change transaction.');
    }
    if (state.store.deleted) {
      throw stateError('The object store has been deleted.');
    }
    requireActive(transaction);
  }

  /** IDBObjectStore.deleteIndex's steps. */
  function deleteIndexOn(handle, name) {
    const state = stateOf(handle, IDBObjectStore);
    requireUpgradeOnStore(state);
    const index = state.store.indexes.get(name);
    if (!index) {
      throw domError('NotFoundError', 'The specified index was not found.');
    }
    removeIndex(state.transaction, index);
  }

  /** IDBObjectStore.index's steps. */
  function indexHandleFor(handle, name) {
    const state = stateOf(handle, IDBObjectStore);
    if (state.store.deleted || state.transaction.state === 'finished') {
      throw stateError('The object store has been deleted or its transaction has finished.');
    }
    const index = state.store.indexes.get(name);
    if (!index) {
      throw domError('NotFoundError', 'The specified index was not found.');
    }
    return indexHandle(handle, index);
  }

  /** IDBIndex's name setter. */
  function renameIndexHandle(handle, name) {
    const state = stateOf(handle, IDBIndex);
    const transaction = state.storeState.transaction;
    if (transaction.mode !== 'versionchange') {
      throw stateError('The database is not running a version change transaction.');
    }
    if (state.index.deleted || state.storeState.store.deleted) {
      throw stateError('The index or its object store has been deleted.');
    }
    requireActive(transaction);
    if (state.index.name === name) {
      return;
    }
    if (state.index.store.indexes.has(name)) {
      throw domError('ConstraintError', 'An index with the specified name already exists.');
    }
    renameIndex(transaction, state.index, name);
  }
