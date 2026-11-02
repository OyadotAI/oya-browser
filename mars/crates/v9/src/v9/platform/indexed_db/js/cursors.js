
  /**
   * Cursors. "Iterate a cursor" is a binary search: every condition the specification puts on
   * the next record (past the current position, at or past a continue() key, at or past a
   * continuePrimaryKey() pair) is a bound on the (key, primary key) order, so the next record is
   * the first item past the tightest lower bound (or the last before the tightest upper bound,
   * going backwards). Searching from the position each time makes cursors see the store as it
   * is now, including records written or deleted since the last step.
   */

  const IDBCursor = makeInterface('IDBCursor', null);
  const IDBCursorWithValue = makeInterface('IDBCursorWithValue', IDBCursor);
  /** Sorts before every primary key. */
  const MIN_KEY = '';
  /** Sorts after every primary key. */
  const MAX_KEY = '￿';

  /** Open a cursor over a store handle. */
  function openStoreCursor(handle, query, direction, keyOnly, operation) {
    const state = liveStore(handle);
    const chosen = directionArgument(direction, operation, 'IDBObjectStore');
    return openCursor(state.transaction, handle, state.store, state.store, toRange(query, false), chosen, keyOnly);
  }

  /** Open a cursor over an index handle. */
  function openIndexCursor(handle, query, direction, keyOnly, operation) {
    const state = liveIndex(handle);
    const chosen = directionArgument(direction, operation, 'IDBIndex');
    return openCursor(state.storeState.transaction, handle, state.index, state.index.store, toRange(query, false), chosen, keyOnly);
  }

  /** Create the cursor and queue its first iteration on a new request. */
  function openCursor(transaction, source, target, store, range, direction, keyOnly) {
    const cursor = {
      brand: IDBCursor, transaction, source, target, store, range, direction, keyOnly,
      position: null, objectStorePosition: null, key: null, primaryKey: null, value: undefined,
      gotValue: false, keyCache: undefined, primaryKeyCache: undefined,
    };
    cursor.object = plainInstance(keyOnly ? IDBCursor : IDBCursorWithValue, cursor);
    const request = newRequest(source, transaction, false);
    cursor.request = request;
    queueRequest(transaction, source, () => iterate(cursor, null, null, 1), request);
    return request.object;
  }

  /** Compare an item with a (key, primary key) bound. */
  function compareBound(item, key, primaryKey) {
    const itemKeyValue = itemKey(item);
    if (itemKeyValue !== key) {
      return itemKeyValue < key ? -1 : 1;
    }
    const primary = itemPrimaryKey(item);
    return primary < primaryKey ? -1 : primary > primaryKey ? 1 : 0;
  }

  /** The bounds an iteration step must respect, given the position and the continue arguments. */
  function stepBounds(cursor, key, primaryKey) {
    const forward = cursor.direction === 'next' || cursor.direction === 'nextunique';
    const unique = cursor.direction === 'nextunique' || cursor.direction === 'prevunique';
    const bounds = [];
    if (cursor.position !== null) {
      const edge = unique ? (forward ? MAX_KEY : MIN_KEY) : cursor.objectStorePosition;
      bounds.push([cursor.position, edge, false]);
    }
    if (key !== null) {
      bounds.push([key, primaryKey !== null ? primaryKey : forward ? MIN_KEY : MAX_KEY, true]);
    }
    return { forward, unique, bounds };
  }

  /** The next item in the cursor's direction, or undefined. */
  function nextItem(cursor, key, primaryKey) {
    const items = itemsOf(cursor.target);
    const [start, end] = spanOf(items, cursor.range, itemKey);
    const { forward, unique, bounds } = stepBounds(cursor, key, primaryKey);
    if (forward) {
      const first = firstPast(items, start, bounds);
      return first < end ? items[first] : undefined;
    }
    const last = lastBefore(items, end, bounds);
    if (last < start) {
      return undefined;
    }
    const found = items[last];
    return unique ? items[Math.max(start, firstWhere(items, (item) => itemKey(item) >= itemKey(found)))] : found;
  }

  /** The first position at or after `start` that is past every lower bound. */
  function firstPast(items, start, bounds) {
    let first = start;
    for (const [boundKey, boundPrimary, inclusive] of bounds) {
      first = Math.max(first, firstWhere(items, (item) => compareBound(item, boundKey, boundPrimary) >= (inclusive ? 0 : 1)));
    }
    return first;
  }

  /** The last position before `end` that is before every upper bound (end - 1 at most). */
  function lastBefore(items, end, bounds) {
    let last = end;
    for (const [boundKey, boundPrimary, inclusive] of bounds) {
      last = Math.min(last, firstWhere(items, (item) => compareBound(item, boundKey, boundPrimary) >= (inclusive ? 1 : 0)));
    }
    return last - 1;
  }

  /** "Iterate a cursor" `count` times; returns the cursor object, or null past the end. */
  function iterate(cursor, key, primaryKey, count) {
    let found;
    for (let remaining = count; remaining > 0; remaining--) {
      found = nextItem(cursor, key, primaryKey);
      if (!found) {
        return exhaust(cursor);
      }
      cursor.position = itemKey(found);
      cursor.objectStorePosition = itemPrimaryKey(found);
    }
    cursor.key = cursor.position;
    cursor.primaryKey = cursor.objectStorePosition;
    cursor.value = cursor.keyOnly ? undefined : cloneValue(itemValue(found));
    cursor.keyCache = undefined;
    cursor.primaryKeyCache = undefined;
    cursor.gotValue = true;
    return cursor.object;
  }

  /** A cursor past its last record: no position, key or value. */
  function exhaust(cursor) {
    cursor.position = null;
    cursor.key = null;
    cursor.value = undefined;
    cursor.gotValue = false;
    return null;
  }
