
  /**
   * IDBCursor and IDBCursorWithValue members. continue(), continuePrimaryKey() and advance()
   * reuse the cursor's request: it goes back to pending and its next success carries the cursor
   * again, or null at the end.
   */

  /** The cursor's state after the active and deleted checks every method starts with. */
  function liveCursor(cursor) {
    const state = stateOf(cursor, IDBCursor);
    requireActive(state.transaction);
    if (state.store.deleted || state.target.deleted) {
      throw stateError('The cursor\'s source or effective object store has been deleted.');
    }
    return state;
  }

  /** Throw unless the cursor is positioned on a record. */
  function requireValue(state) {
    if (!state.gotValue) {
      throw stateError('The cursor is being iterated or has iterated past its end.');
    }
  }

  /** Reuse the cursor's request for its next iteration. */
  function requeue(state, key, primaryKey, count) {
    state.gotValue = false;
    state.request.done = false;
    state.request.result = undefined;
    state.request.error = null;
    queueRequest(state.transaction, state.source, () => iterate(state, key, primaryKey, count), state.request);
  }

  /** Whether `key` fails to move past the position in the cursor's direction. */
  function behind(state, key) {
    const forward = state.direction === 'next' || state.direction === 'nextunique';
    return forward ? key <= state.position : key >= state.position;
  }

  /** The script value of an encoded key, cached so repeated reads give the same object. */
  function cachedKey(state, field, cache) {
    if (state[cache] === undefined) {
      state[cache] = state[field] === null ? undefined : keyValue(state[field]);
    }
    return state[cache];
  }

  members(IDBCursor.prototype, {
    get source() { return stateOf(this, IDBCursor).source; },
    get direction() { return stateOf(this, IDBCursor).direction; },
    get key() { return cachedKey(stateOf(this, IDBCursor), 'key', 'keyCache'); },
    get primaryKey() { return cachedKey(stateOf(this, IDBCursor), 'primaryKey', 'primaryKeyCache'); },
    get request() { return stateOf(this, IDBCursor).request.object; },
    advance(count) {
      requireArgument(arguments, 1, 'advance', 'IDBCursor');
      const steps = countArgument(count);
      if (steps === 0) {
        throw new NativeTypeError("Failed to execute 'advance' on 'IDBCursor': A count argument with value 0 (zero) was supplied, must be greater than 0.");
      }
      const state = liveCursor(this);
      requireValue(state);
      requeue(state, null, null, steps);
    },
    continue(key = undefined) {
      const state = liveCursor(this);
      requireValue(state);
      const target = key === undefined ? null : requireKey(key);
      if (target !== null && behind(state, target)) {
        throw dataError('The parameter is less than or equal to this cursor\'s position.');
      }
      requeue(state, target, null, 1);
    },
    continuePrimaryKey(key, primaryKey) {
      requireArgument(arguments, 2, 'continuePrimaryKey', 'IDBCursor');
      const state = liveCursor(this);
      checkPrimaryContinue(state);
      const target = requireKey(key);
      const primary = requireKey(primaryKey);
      checkPrimaryOrder(state, target, primary);
      requeue(state, target, primary, 1);
    },
    update(value) {
      requireArgument(arguments, 1, 'update', 'IDBCursor');
      const state = writableCursor(this);
      const clone = cloneInTransaction(state.transaction, value);
      if (state.store.keyPath !== null && extractKey(clone, state.store.keyPath) !== state.primaryKey) {
        throw dataError('The effective object store of this cursor uses in-line keys and evaluating the key path of the value parameter results in a different value than the cursor\'s effective key.');
      }
      const key = state.primaryKey;
      return queueRequest(state.transaction, this, () => storeRecord(state.transaction, state.store, clone, key, false)).object;
    },
    delete() {
      const state = writableCursor(this);
      const key = state.primaryKey;
      const range = rangeRecord(key, key, false, false);
      return queueRequest(state.transaction, this, () => deleteRange(state.transaction, state.store, range)).object;
    },
  });

  members(IDBCursorWithValue.prototype, {
    get value() { return stateOf(this, IDBCursor).value; },
  });

  /** The checks update() and delete() make, in the specification's order. */
  function writableCursor(cursor) {
    const state = stateOf(cursor, IDBCursor);
    requireActive(state.transaction);
    requireWritable(state.transaction);
    if (state.store.deleted || state.target.deleted) {
      throw stateError('The cursor\'s source or effective object store has been deleted.');
    }
    requireValue(state);
    if (state.keyOnly) {
      throw stateError('The cursor is a key cursor.');
    }
    return state;
  }

  /** continuePrimaryKey() needs an index source and a non-unique direction. */
  function checkPrimaryContinue(state) {
    if (!state.target.store) {
      throw domError('InvalidAccessError', 'The cursor\'s source is not an index.');
    }
    if (state.direction !== 'next' && state.direction !== 'prev') {
      throw domError('InvalidAccessError', 'The cursor\'s direction is not \'next\' or \'prev\'.');
    }
    requireValue(state);
  }

  /** continuePrimaryKey()'s target must lie past the position in the cursor's direction. */
  function checkPrimaryOrder(state, key, primaryKey) {
    const forward = state.direction === 'next';
    const before = forward ? key < state.position : key > state.position;
    const samePast = key === state.position
      && (forward ? primaryKey <= state.objectStorePosition : primaryKey >= state.objectStorePosition);
    if (before || samePast) {
      throw dataError('The parameters are not past this cursor\'s position.');
    }
  }
