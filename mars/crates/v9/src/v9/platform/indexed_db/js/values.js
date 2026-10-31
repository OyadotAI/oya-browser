
  /**
   * Values. A stored value is a structured clone the store owns; every read hands out a fresh
   * clone. For persistence a stored value is written as JSON text of a tagged graph: objects are
   * numbered in the order first met, so shared and cyclic references come back as the same
   * objects. JSON.stringify escapes lone surrogates, so the text is always well-formed and
   * survives the coordinator's UTF-8 storage.
   */

  /** StructuredSerializeForStorage then deserialize: the copy the store keeps or hands out. */
  function cloneValue(value) {
    return structuredCloneNative(value);
  }

  /** Numbers JSON cannot carry, written as tagged strings. */
  const SPECIAL_NUMBERS = ['NaN', 'Infinity', '-Infinity', '-0'];

  /** Serialize a stored value as JSON text. */
  function serializeValue(value) {
    return jsonStringify(encodeNode(value, new NativeMap()));
  }

  /** The JSON-able node of one value; `ids` numbers the objects already written. */
  function encodeNode(value, ids) {
    switch (typeof value) {
      case 'undefined':
        return ['u'];
      case 'number':
        return encodeNumberNode(value);
      case 'bigint':
        return ['i', String(value)];
      case 'object':
        return value === null ? null : encodeObject(value, ids);
      default:
        return value;
    }
  }

  /** A number node: plain unless JSON would lose it. */
  function encodeNumberNode(number) {
    if (Number.isFinite(number) && !Object.is(number, -0)) {
      return number;
    }
    return ['n', Object.is(number, -0) ? '-0' : String(number)];
  }

  /** An object node, or a reference to one already written. */
  function encodeObject(object, ids) {
    const known = call(mapGet, ids, object);
    if (known !== undefined) {
      return ['r', known];
    }
    ids.set(object, ids.size);
    return objectEncoder(object)(object, ids);
  }

  /** Pick the encoder for an object's kind, checked by brand rather than by its prototype. */
  function objectEncoder(object) {
    if (timeOf(object) !== undefined) {
      return (date) => ['D', encodeNumberNode(timeOf(date))];
    }
    if (host.blob(object)) {
      return encodeBlob;
    }
    if (isView(object)) {
      return encodeView;
    }
    if (bufferLength(object) !== undefined) {
      return (buffer) => ['B', base64(new NativeUint8Array(buffer))];
    }
    return kindEncoder(object);
  }

  /** Encoders for the remaining kinds, told apart by their [[Class]]-like tag. */
  function kindEncoder(object) {
    const tag = call(objectToString, object);
    if (isArray(object)) {
      return (array, ids) => ['A', array.length, encodeFields(array, ids)];
    }
    if (tag === '[object Map]') {
      return (map, ids) => ['M', [...map].map(([key, value]) => [encodeNode(key, ids), encodeNode(value, ids)])];
    }
    if (tag === '[object Set]') {
      return (set, ids) => ['S', [...set].map((value) => encodeNode(value, ids))];
    }
    if (tag === '[object RegExp]') {
      return (regexp) => ['R', call(regexpSource, regexp), call(regexpFlags, regexp)];
    }
    return wrapperEncoder(tag) || ((plain, ids) => ['O', encodeFields(plain, ids)]);
  }

  /** Encoders for Boolean, Number, String and BigInt objects and errors. */
  function wrapperEncoder(tag) {
    const wrappers = {
      '[object Boolean]': (box) => ['W', 'Boolean', call(Boolean.prototype.valueOf, box)],
      '[object Number]': (box) => ['W', 'Number', encodeNumberNode(call(Number.prototype.valueOf, box))],
      '[object String]': (box) => ['W', 'String', call(String.prototype.valueOf, box)],
      '[object BigInt]': (box) => ['W', 'BigInt', String(call(BigInt.prototype.valueOf, box))],
      '[object Error]': (error) => ['E', String(error.name), String(error.message)],
    };
    return own(wrappers, tag) ? wrappers[tag] : null;
  }

  /** Own enumerable string-keyed fields, in order, as [name, node] pairs. */
  function encodeFields(object, ids) {
    return objectKeys(object).map((name) => [name, encodeNode(object[name], ids)]);
  }

  /** A typed array or DataView node: its kind, its (shared) buffer node, offset and length. */
  function encodeView(view, ids) {
    const kind = call(typedTag, view);
    if (kind === undefined) {
      return ['V', 'DataView', encodeNode(call(dataViewBuffer, view), ids), call(dataViewOffset, view), call(dataViewLength, view)];
    }
    return ['V', kind, encodeNode(call(viewBuffer, view), ids), call(viewByteOffset, view), view.length];
  }

  /** A Blob or File node with its bytes. */
  function encodeBlob(blob) {
    const [type, bytes, name, modified] = host.blob(blob);
    return name === undefined ? ['L', type, base64(bytes)] : ['F', type, base64(bytes), name, modified];
  }

  /** An ArrayBuffer's byte length, or undefined for any other value. */
  function bufferLength(value) {
    try {
      return call(bufferByteLength, value);
    } catch {
      return undefined;
    }
  }

  /** Bytes per base64 chunk, so fromCharCode never receives an oversized argument list. */
  const BASE64_CHUNK = 8192;

  /** Base64 of a byte array. */
  function base64(bytes) {
    let latin1 = '';
    for (let start = 0; start < bytes.length; start += BASE64_CHUNK) {
      latin1 += apply(fromCharCode, null, bytes.subarray(start, start + BASE64_CHUNK));
    }
    return btoaNative(latin1);
  }

  /** The bytes a base64 string holds. */
  function fromBase64(text) {
    const latin1 = atobNative(text);
    const bytes = new NativeUint8Array(latin1.length);
    for (let index = 0; index < latin1.length; index++) {
      bytes[index] = latin1.charCodeAt(index);
    }
    return bytes;
  }
