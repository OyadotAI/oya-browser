
  /**
   * Key paths: validation, evaluation against a stored value, and injection of a generated key.
   * A key path is null, a string, or an array of strings (kept as a frozen array internally).
   */

  /** One IdentifierName, as ECMAScript defines it. */
  const IDENTIFIER = /^[$_\p{ID_Start}][$‌‍\p{ID_Continue}]*$/u;
  /** Blob members a key path may read, beside its own properties. */
  const BLOB_MEMBERS = ['size', 'type'];
  /** File members a key path may read. */
  const FILE_MEMBERS = ['name', 'lastModified'];

  /** WebIDL's (DOMString or sequence<DOMString>) conversion of a keyPath argument. */
  function keyPathArgument(value) {
    if (isObject(value) && typeof value[Symbol.iterator] === 'function' && typeof value !== 'string') {
      return Object.freeze([...value].map((item) => String(item)));
    }
    return String(value);
  }

  /** "Valid key path": the empty string, dot-separated identifiers, or a non-empty array of valid string paths. */
  function validKeyPath(path) {
    if (typeof path === 'string') {
      return path === '' || path.split('.').every((part) => IDENTIFIER.test(part));
    }
    return path.length > 0 && path.every((part) => typeof part === 'string' && validKeyPath(part));
  }

  /** The script value of a key path: a string, or a new array each time the caller caches it. */
  function keyPathValue(path) {
    return typeof path === 'string' || path === null ? path : [...path];
  }

  /** Marks a key path that found no value. */
  const NO_VALUE = Symbol('no value');

  /** "Evaluate a key path on a value": the value found, or NO_VALUE. */
  function evaluateKeyPath(value, path) {
    if (typeof path !== 'string') {
      const results = [];
      for (const part of path) {
        const result = evaluateKeyPath(value, part);
        if (result === NO_VALUE) {
          return NO_VALUE;
        }
        results.push(result);
      }
      return results;
    }
    return path === '' ? value : evaluateIdentifiers(value, path.split('.'));
  }

  /** Walk identifiers from `value`; strings and arrays answer `length`, Blobs and Files their attributes. */
  function evaluateIdentifiers(value, identifiers) {
    for (const identifier of identifiers) {
      if ((typeof value === 'string' || isArray(value)) && identifier === 'length') {
        value = value.length;
      } else if (blobMember(value, identifier)) {
        value = value[identifier];
      } else if (!isObject(value) || !own(value, identifier)) {
        return NO_VALUE;
      } else {
        value = value[identifier];
      }
    }
    return value;
  }

  /** Whether a key path identifier names a Blob or File attribute of `value`. */
  function blobMember(value, identifier) {
    if (!isObject(value) || !host.blob(value)) {
      return false;
    }
    return BLOB_MEMBERS.includes(identifier) || (FILE_MEMBERS.includes(identifier) && value instanceof NativeFile);
  }

  /** "Extract a key from a value using a key path": the encoded key, null for an invalid key, or NO_VALUE. */
  function extractKey(value, path) {
    const result = evaluateKeyPath(value, path);
    return result === NO_VALUE ? NO_VALUE : valueToKey(result);
  }

  /** "Check that a key could be injected into a value". */
  function canInjectKey(value, path) {
    const identifiers = path.split('.');
    identifiers.pop();
    for (const identifier of identifiers) {
      if (!isObject(value)) {
        return false;
      }
      if (!own(value, identifier)) {
        return true;
      }
      value = value[identifier];
    }
    return isObject(value);
  }

  /** "Inject a key into a value using a key path", creating plain objects for missing steps. */
  function injectKey(value, key, path) {
    const identifiers = path.split('.');
    const last = identifiers.pop();
    for (const identifier of identifiers) {
      if (!own(value, identifier)) {
        defineData(value, identifier, {});
      }
      value = value[identifier];
    }
    defineData(value, last, keyValue(key));
  }

  /** CreateDataProperty: an enumerable, writable, configurable own property. */
  function defineData(object, name, value) {
    defineProperty(object, name, { value, writable: true, enumerable: true, configurable: true });
  }
