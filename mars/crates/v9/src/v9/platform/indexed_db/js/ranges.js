
  /**
   * IDBKeyRange. Internally a range is { lower, upper, lowerOpen, upperOpen } over encoded keys,
   * with null for an unbounded side; the interface object wraps one.
   */

  const IDBKeyRange = makeInterface('IDBKeyRange', null);

  /** A range record; both bounds are encoded keys or null. */
  function rangeRecord(lower, upper, lowerOpen, upperOpen) {
    return { lower, upper, lowerOpen: Boolean(lowerOpen), upperOpen: Boolean(upperOpen) };
  }

  /** The range that contains every key. */
  const UNBOUNDED = rangeRecord(null, null, false, false);

  /** Wrap a range record as an IDBKeyRange object. */
  function keyRangeObject(range) {
    return plainInstance(IDBKeyRange, { brand: IDBKeyRange, range });
  }

  /** "Key in range": whether an encoded key lies inside the range. */
  function inRange(range, key) {
    if (range.lower !== null) {
      const order = compareKeys(range.lower, key);
      if (order > 0 || (order === 0 && range.lowerOpen)) {
        return false;
      }
    }
    if (range.upper !== null) {
      const order = compareKeys(range.upper, key);
      if (order < 0 || (order === 0 && range.upperOpen)) {
        return false;
      }
    }
    return true;
  }

  /** "Convert a value to a key range"; `nullDisallowed` makes undefined and null a DataError. */
  function toRange(value, nullDisallowed) {
    if (isObject(value)) {
      const state = apply(weakGet, internals, [value]);
      if (state && state.brand === IDBKeyRange) {
        return state.range;
      }
    }
    if (value === undefined || value === null) {
      if (nullDisallowed) {
        throw dataError('No key or key range specified.');
      }
      return UNBOUNDED;
    }
    const key = requireKey(value);
    return rangeRecord(key, key, false, false);
  }

  members(IDBKeyRange.prototype, {
    get lower() { return boundValue(stateOf(this, IDBKeyRange).range.lower); },
    get upper() { return boundValue(stateOf(this, IDBKeyRange).range.upper); },
    get lowerOpen() { return stateOf(this, IDBKeyRange).range.lowerOpen; },
    get upperOpen() { return stateOf(this, IDBKeyRange).range.upperOpen; },
    includes(key) {
      const range = stateOf(this, IDBKeyRange).range;
      return inRange(range, requireKey(key));
    },
  });

  /** A bound as scripts see it: the decoded key, or undefined when unbounded. */
  function boundValue(key) {
    return key === null ? undefined : keyValue(key);
  }

  members(IDBKeyRange, {
    only(value) {
      const key = requireKey(value);
      return keyRangeObject(rangeRecord(key, key, false, false));
    },
    lowerBound(lower, open = false) {
      return keyRangeObject(rangeRecord(requireKey(lower), null, open, true));
    },
    upperBound(upper, open = false) {
      return keyRangeObject(rangeRecord(null, requireKey(upper), true, open));
    },
    bound(lower, upper, lowerOpen = false, upperOpen = false) {
      const low = requireKey(lower);
      const high = requireKey(upper);
      const order = compareKeys(low, high);
      if (order > 0 || (order === 0 && (lowerOpen || upperOpen))) {
        throw dataError('The lower key is greater than the upper key.');
      }
      return keyRangeObject(rangeRecord(low, high, lowerOpen, upperOpen));
    },
  });
