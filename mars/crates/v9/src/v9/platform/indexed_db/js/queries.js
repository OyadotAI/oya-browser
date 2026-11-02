
  /**
   * Retrieval shared by object stores and indexes. A "source" is a store model (records sorted by
   * key) or an index model (entries sorted by index key, then primary key); these helpers read
   * either one the same way.
   */

  /** Cursor and getAll directions. */
  const DIRECTIONS = ['next', 'nextunique', 'prev', 'prevunique'];
  /** The largest count WebIDL's [EnforceRange] unsigned long admits. */
  const MAX_COUNT = 4294967295;

  /** The sorted items of a source. */
  function itemsOf(source) {
    return source.store ? source.entries : source.records;
  }

  /** An item's key in its source's order: the index key for an index, the primary key for a store. */
  function itemKey(item) {
    return item.ik === undefined ? item.key : item.ik;
  }

  /** An item's primary key. */
  function itemPrimaryKey(item) {
    return item.ik === undefined ? item.key : item.pk;
  }

  /** An item's stored value. */
  function itemValue(item) {
    return item.ik === undefined ? item.value : item.record.value;
  }

  /** Validate a direction argument, as the IDBCursorDirection enum conversion does. */
  function directionArgument(value, operation, interfaceName) {
    const direction = value === undefined ? 'next' : String(value);
    if (!DIRECTIONS.includes(direction)) {
      throw new NativeTypeError(`Failed to execute '${operation}' on '${interfaceName}': The provided value '${direction}' is not a valid enum value of type IDBCursorDirection.`);
    }
    return direction;
  }

  /** WebIDL's [EnforceRange] unsigned long for getAll's count; undefined and 0 mean "all". */
  function countArgument(value) {
    if (value === undefined) {
      return 0;
    }
    const number = Number(value);
    if (!Number.isFinite(number) || Math.trunc(number) < 0 || Math.trunc(number) > MAX_COUNT) {
      throw new NativeTypeError('Value is outside the \'unsigned long\' value range.');
    }
    return Math.trunc(number);
  }

  /** The items in a range, in a direction, at most `count` of them (0 for all). Unique directions keep the lowest primary key of each key. */
  function collect(source, range, count, direction) {
    const items = itemsOf(source);
    const [start, end] = spanOf(items, range, itemKey);
    const unique = direction === 'nextunique' || direction === 'prevunique';
    const picked = [];
    const forward = direction === 'next' || direction === 'nextunique';
    for (let step = 0; step < end - start && (count === 0 || picked.length < count); step++) {
      const at = forward ? start + step : end - 1 - step;
      if (!unique || at === start || itemKey(items[at - 1]) !== itemKey(items[at])) {
        picked.push(items[at]);
      }
    }
    return picked;
  }

  /** The first item in a range, or undefined. */
  function firstInRange(source, range) {
    const items = itemsOf(source);
    const [start, end] = spanOf(items, range, itemKey);
    return start < end ? items[start] : undefined;
  }

  /** How many items lie in a range. */
  function countInRange(source, range) {
    const [start, end] = spanOf(itemsOf(source), range, itemKey);
    return end - start;
  }

  /** "Is a potentially valid key range": what getAll's first argument must be to be a query rather than options. */
  function potentiallyRange(value) {
    if (!isObject(value)) {
      return true;
    }
    const state = apply(weakGet, internals, [value]);
    return (state && state.brand === IDBKeyRange) || isArray(value) || timeOf(value) !== undefined || bytesOf(value) !== undefined;
  }

  /** getAll's arguments: (query, count) or an IDBGetAllOptions dictionary. */
  function getAllArguments(queryOrOptions, count, operation, interfaceName) {
    if (potentiallyRange(queryOrOptions)) {
      return { range: toRange(queryOrOptions, false), count: countArgument(count), direction: 'next' };
    }
    const options = queryOrOptions;
    return {
      range: toRange(options.query, false),
      count: countArgument(options.count),
      direction: directionArgument(options.direction, operation, interfaceName),
    };
  }

  /** IDBRecord: one key, primary key and value from getAllRecords. */
  const IDBRecord = makeInterface('IDBRecord', null);
  members(IDBRecord.prototype, {
    get key() { return stateOf(this, IDBRecord).key; },
    get primaryKey() { return stateOf(this, IDBRecord).primaryKey; },
    get value() { return stateOf(this, IDBRecord).value; },
  });

  /** An IDBRecord for an item. */
  function recordObject(item) {
    return plainInstance(IDBRecord, {
      brand: IDBRecord, key: keyValue(itemKey(item)), primaryKey: keyValue(itemPrimaryKey(item)), value: cloneValue(itemValue(item)),
    });
  }

  /** The request operations getAll, getAllKeys and getAllRecords share. */
  const ALL_RESULTS = {
    values: (items) => items.map((item) => cloneValue(itemValue(item))),
    keys: (items) => items.map((item) => keyValue(itemPrimaryKey(item))),
    records: (items) => items.map(recordObject),
  };
