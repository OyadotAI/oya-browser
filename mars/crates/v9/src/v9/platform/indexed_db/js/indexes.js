
  /**
   * IDBIndex: a store handle's view of one index. Reads walk the index's entries, ordered by
   * index key then primary key, and answer with the referenced record's value or primary key.
   */

  const IDBIndex = makeInterface('IDBIndex', null);

  /** The one IDBIndex object a store handle hands out for an index. */
  function indexHandle(storeHandle, index) {
    const storeState = stateOf(storeHandle, IDBObjectStore);
    let handle = call(mapGet, storeState.indexHandles, index);
    if (!handle) {
      handle = plainInstance(IDBIndex, { brand: IDBIndex, storeHandle, storeState, index, keyPath: undefined });
      storeState.indexHandles.set(index, handle);
    }
    return handle;
  }

  /** The handle's state after the deleted and active-transaction checks. */
  function liveIndex(handle) {
    const state = stateOf(handle, IDBIndex);
    if (state.index.deleted || state.storeState.store.deleted) {
      throw stateError('The index or its object store has been deleted.');
    }
    requireActive(state.storeState.transaction);
    return state;
  }

  members(IDBIndex.prototype, {
    get name() { return stateOf(this, IDBIndex).index.name; },
    set name(value) { renameIndexHandle(this, String(value)); },
    get objectStore() { return stateOf(this, IDBIndex).storeHandle; },
    get keyPath() {
      const state = stateOf(this, IDBIndex);
      if (state.keyPath === undefined) {
        state.keyPath = keyPathValue(state.index.keyPath);
      }
      return state.keyPath;
    },
    get multiEntry() { return stateOf(this, IDBIndex).index.multiEntry; },
    get unique() { return stateOf(this, IDBIndex).index.unique; },
    get(query) {
      requireArgument(arguments, 1, 'get', 'IDBIndex');
      return indexRead(this, query, (item) => (item ? cloneValue(item.record.value) : undefined));
    },
    getKey(query) {
      requireArgument(arguments, 1, 'getKey', 'IDBIndex');
      return indexRead(this, query, (item) => (item ? keyValue(item.pk) : undefined));
    },
    getAll(queryOrOptions = undefined, count = undefined) { return indexAll(this, queryOrOptions, count, 'getAll', ALL_RESULTS.values); },
    getAllKeys(queryOrOptions = undefined, count = undefined) { return indexAll(this, queryOrOptions, count, 'getAllKeys', ALL_RESULTS.keys); },
    getAllRecords(options = undefined) { return indexAll(this, options === undefined ? {} : options, undefined, 'getAllRecords', ALL_RESULTS.records); },
    count(query = undefined) {
      const state = liveIndex(this);
      const range = toRange(query, false);
      return queueRequest(state.storeState.transaction, this, () => countInRange(state.index, range)).object;
    },
    openCursor(query = undefined, direction = undefined) { return openIndexCursor(this, query, direction, false, 'openCursor'); },
    openKeyCursor(query = undefined, direction = undefined) { return openIndexCursor(this, query, direction, true, 'openKeyCursor'); },
  });

  /** Queue a read of the first entry in a range. */
  function indexRead(handle, query, answer) {
    const state = liveIndex(handle);
    const range = toRange(query, true);
    return queueRequest(state.storeState.transaction, handle, () => answer(firstInRange(state.index, range))).object;
  }

  /** Queue a getAll-style read over the index. */
  function indexAll(handle, queryOrOptions, count, operation, answer) {
    const state = liveIndex(handle);
    const query = getAllArguments(queryOrOptions, count, operation, 'IDBIndex');
    return queueRequest(state.storeState.transaction, handle, () => answer(collect(state.index, query.range, query.count, query.direction))).object;
  }
