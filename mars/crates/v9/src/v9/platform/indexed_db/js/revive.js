
  /**
   * Reviving persisted values: the inverse of serializeValue. Object nodes are created before
   * their fields are read and numbered in the same order the writer numbered them, so a
   * reference node always names an object that already exists.
   */

  /** Typed array constructors by the name their Symbol.toStringTag reports. */
  const VIEW_CONSTRUCTORS = {
    Int8Array, Uint8Array, Uint8ClampedArray, Int16Array, Uint16Array, Int32Array, Uint32Array,
    Float32Array, Float64Array, BigInt64Array, BigUint64Array, DataView,
  };
  /** Error constructors a persisted error may name; any other name revives as Error. */
  const ERROR_CONSTRUCTORS = { Error, EvalError, RangeError, ReferenceError, SyntaxError, TypeError, URIError };
  /** Primitive wrapper revivers by kind. */
  const WRAPPERS = {
    Boolean: (value) => Object(Boolean(value)),
    Number: (value) => Object(reviveNumber(value)),
    String: (value) => Object(String(value)),
    BigInt: (value) => Object(BigInt(value)),
  };

  /** Revive a value from its JSON text. */
  function deserializeValue(text) {
    return reviveNode(jsonParse(text), []);
  }

  /** Revive one node; `objects` holds every object revived so far, by number. */
  function reviveNode(node, objects) {
    if (!isArray(node)) {
      return node;
    }
    const tag = node[0];
    if (tag === 'u') {
      return undefined;
    }
    if (tag === 'n') {
      return reviveNumber(node);
    }
    if (tag === 'i') {
      return BigInt(node[1]);
    }
    if (tag === 'r') {
      return objects[node[1]];
    }
    return reviveObject(node, objects);
  }

  /** A number node: plain, or one of the tagged special values. */
  function reviveNumber(node) {
    if (!isArray(node)) {
      return node;
    }
    return node[1] === '-0' ? -0 : Number(node[1]);
  }

  /** Revive an object node, registering it before its contents. */
  function reviveObject(node, objects) {
    const reviver = OBJECT_REVIVERS[node[0]];
    if (!reviver) {
      throw domError('DataError', 'A stored value could not be read.');
    }
    return reviver(node, objects);
  }

  /** Register an object under the next number and return it. */
  function register(objects, object) {
    objects.push(object);
    return object;
  }

  /** Revivers by object tag. */
  const OBJECT_REVIVERS = {
    O: (node, objects) => fill(register(objects, {}), node[1], objects),
    A: (node, objects) => fillArray(node, objects),
    D: (node, objects) => register(objects, new NativeDate(reviveNumber(node[1]))),
    R: (node, objects) => register(objects, new RegExp(node[1], node[2])),
    M: (node, objects) => reviveMap(node, objects),
    S: (node, objects) => reviveSet(node, objects),
    B: (node, objects) => register(objects, fromBase64(node[1]).buffer),
    V: (node, objects) => reviveView(node, objects),
    W: (node, objects) => register(objects, WRAPPERS[node[1]](node[2])),
    E: (node, objects) => register(objects, reviveError(node)),
    L: (node, objects) => register(objects, new NativeBlob([fromBase64(node[2])], { type: node[1] })),
    F: (node, objects) => register(objects, new NativeFile([fromBase64(node[2])], node[3], { type: node[1], lastModified: node[4] })),
  };

  /** Define revived fields as plain data properties. */
  function fill(object, fields, objects) {
    for (const [name, field] of fields) {
      defineData(object, name, reviveNode(field, objects));
    }
    return object;
  }

  /** An array of the recorded length with its fields. */
  function fillArray(node, objects) {
    const array = register(objects, []);
    array.length = node[1];
    return fill(array, node[2], objects);
  }

  /** A Map with its entries in order. */
  function reviveMap(node, objects) {
    const map = register(objects, new NativeMap());
    for (const [key, value] of node[1]) {
      map.set(reviveNode(key, objects), reviveNode(value, objects));
    }
    return map;
  }

  /** A Set with its values in order. */
  function reviveSet(node, objects) {
    const set = register(objects, new NativeSet());
    for (const value of node[1]) {
      set.add(reviveNode(value, objects));
    }
    return set;
  }

  /** A view over its revived (possibly shared) buffer. The view is numbered before its buffer, as it was written. */
  function reviveView(node, objects) {
    const index = objects.length;
    objects.push(null);
    const buffer = reviveNode(node[2], objects);
    const View = own(VIEW_CONSTRUCTORS, node[1]) ? VIEW_CONSTRUCTORS[node[1]] : NativeUint8Array;
    objects[index] = new View(buffer, node[3], node[4]);
    return objects[index];
  }

  /** An error of the recorded kind and message. */
  function reviveError(node) {
    const Kind = own(ERROR_CONSTRUCTORS, node[1]) ? ERROR_CONSTRUCTORS[node[1]] : Error;
    const error = new Kind(node[2]);
    if (Kind === Error && node[1] !== 'Error') {
      defineProperty(error, 'name', { value: node[1], writable: true, enumerable: false, configurable: true });
    }
    return error;
  }
