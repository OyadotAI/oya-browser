
  /**
   * Keys. Inside this file a key is a string whose code-unit order is the specification's key
   * order (number < date < string < binary < array, each compared as "compare two keys"
   * defines), so comparing, sorting and binary searching keys are plain string comparisons.
   * A key starts with a type tag; numbers and dates are 16 hex digits of an order-preserving
   * transform of their IEEE-754 bits, strings and binaries end in U+0000 with U+0000 and U+0001
   * escaped, and arrays end in U+0000, which sorts before any element's tag, so a prefix sorts
   * first. Decoding a key gives the value scripts see.
   */

  /** Type tags, in key order. */
  const TAG_NUMBER = '1';
  const TAG_DATE = '2';
  const TAG_STRING = '3';
  const TAG_BINARY = '4';
  const TAG_ARRAY = '5';
  /** Ends a string, binary or array key. */
  const KEY_END = '\u0000';
  /** Escapes U+0000 and U+0001 inside string keys, so U+0000 can end them. */
  const KEY_ESCAPE = '\u0001';
  /** Hex digits in one 32-bit half of an encoded number. */
  const HALF_DIGITS = 8;
  /** Hex digits in an encoded number. */
  const NUMBER_DIGITS = 16;
  /** Radix of the encoded number digits. */
  const HEX = 16;
  /** The IEEE-754 sign bit of the high half. */
  const SIGN_BIT = 0x80000000;
  /** Bytes of a float64. */
  const FLOAT_BYTES = 8;
  /** Offset of the low 32-bit half in a float64's big-endian bytes. */
  const LOW_HALF = 4;
  /** Binary key bytes are stored shifted by one so U+0000 can end them. */
  const BYTE_SHIFT = 1;
  /** Scratch space for number encoding. */
  const numberView = new NativeDataView(new NativeArrayBuffer(FLOAT_BYTES));

  /** Eight hex digits of an unsigned 32-bit value. */
  function hex8(value) {
    return value.toString(HEX).padStart(HALF_DIGITS, '0');
  }

  /** A number's order-preserving 16 hex digits: negative numbers have every bit flipped, others only the sign bit. */
  function encodeNumber(number) {
    call(setFloat64, numberView, 0, number === 0 ? 0 : number);
    let high = call(getUint32, numberView, 0);
    let low = call(getUint32, numberView, LOW_HALF);
    if (high & SIGN_BIT) {
      high = ~high >>> 0;
      low = ~low >>> 0;
    } else {
      high = (high | SIGN_BIT) >>> 0;
    }
    return hex8(high) + hex8(low);
  }

  /** The number 16 hex digits at `position` encode. */
  function decodeNumber(key, position) {
    let high = parseInt(key.slice(position, position + HALF_DIGITS), HEX);
    let low = parseInt(key.slice(position + HALF_DIGITS, position + NUMBER_DIGITS), HEX);
    if (high & SIGN_BIT) {
      high = (high & ~SIGN_BIT) >>> 0;
    } else {
      high = ~high >>> 0;
      low = ~low >>> 0;
    }
    call(setUint32, numberView, 0, high);
    call(setUint32, numberView, LOW_HALF, low);
    return call(getFloat64, numberView, 0);
  }

  /** A string's escaped body: U+0000 and U+0001 become two units, so U+0000 never appears inside. */
  function escapeString(text) {
    return text.replace(/[\u0000\u0001]/g, (unit) => KEY_ESCAPE + fromCharCode(unit.charCodeAt(0) + 1));
  }

  /** The time value of a Date, or undefined for any other value. */
  function timeOf(value) {
    try {
      return call(dateGetTime, value);
    } catch {
      return undefined;
    }
  }

  /** The bytes of an ArrayBuffer or view, copied, or undefined for any other value. */
  function bytesOf(value) {
    if (isView(value)) {
      return viewBytes(value);
    }
    try {
      const length = call(bufferByteLength, value);
      return new NativeUint8Array(value, 0, length).slice();
    } catch {
      return undefined;
    }
  }

  /** A view's bytes, copied, whether it is a typed array or a DataView. */
  function viewBytes(view) {
    if (call(typedTag, view) === undefined) {
      return new NativeUint8Array(call(dataViewBuffer, view), call(dataViewOffset, view), call(dataViewLength, view)).slice();
    }
    return new NativeUint8Array(call(viewBuffer, view), call(viewByteOffset, view), call(typedByteLength, view)).slice();
  }

  /** A binary key's body: each byte shifted past U+0000, then the end mark. */
  function encodeBytes(bytes) {
    let body = '';
    for (let index = 0; index < bytes.length; index++) {
      body += fromCharCode(bytes[index] + BYTE_SHIFT);
    }
    return body + KEY_END;
  }

  /**
   * "Convert a value to a key": the encoded key, or null when the value is not a valid key.
   * Array elements are read with [[Get]], so a throwing getter propagates; a sparse array, a
   * cycle, NaN or an invalid Date make the whole key invalid.
   */
  function valueToKey(input, seen) {
    if (typeof input === 'number') {
      return Number.isNaN(input) ? null : TAG_NUMBER + encodeNumber(input);
    }
    if (typeof input === 'string') {
      return TAG_STRING + escapeString(input) + KEY_END;
    }
    if (!isObject(input)) {
      return null;
    }
    const time = timeOf(input);
    if (time !== undefined) {
      return Number.isNaN(time) ? null : TAG_DATE + encodeNumber(time);
    }
    const bytes = bytesOf(input);
    if (bytes !== undefined) {
      return TAG_BINARY + encodeBytes(bytes);
    }
    return isArray(input) ? arrayToKey(input, seen || new NativeSet()) : null;
  }

  /** An array key from a dense, acyclic array of valid keys, or null. */
  function arrayToKey(input, seen) {
    if (call(setHas, seen, input)) {
      return null;
    }
    seen.add(input);
    let key = TAG_ARRAY;
    const length = input.length;
    for (let index = 0; index < length; index++) {
      if (!own(input, index)) {
        return null;
      }
      const element = valueToKey(input[index], seen);
      if (element === null) {
        return null;
      }
      key += element;
    }
    seen.delete(input);
    return key + KEY_END;
  }

  /** "Convert a value to a multiEntry key": an array yields its distinct valid elements, anything else one key or none. */
  function multiEntryKeys(input) {
    if (!isArray(input)) {
      const key = valueToKey(input);
      return key === null ? [] : [key];
    }
    const seen = new NativeSet([input]);
    const keys = new NativeSet();
    for (let index = 0; index < input.length; index++) {
      if (own(input, index)) {
        const key = valueToKey(input[index], seen);
        if (key !== null) {
          keys.add(key);
        }
      }
    }
    return [...keys];
  }

  /** The script value of an encoded key: numbers, strings, new Date and ArrayBuffer objects, and arrays of them. */
  function keyValue(key) {
    return decodeKey(key, 0)[0];
  }

  /** Decode the key at `position`; returns the value and the position after it. */
  function decodeKey(key, position) {
    const tag = key[position];
    const start = position + 1;
    if (tag === TAG_NUMBER) {
      return [decodeNumber(key, start), start + NUMBER_DIGITS];
    }
    if (tag === TAG_DATE) {
      return [new NativeDate(decodeNumber(key, start)), start + NUMBER_DIGITS];
    }
    if (tag === TAG_STRING) {
      return decodeString(key, start);
    }
    if (tag === TAG_BINARY) {
      return decodeBinary(key, start);
    }
    return decodeArray(key, start);
  }

  /** Decode an escaped string body ending in U+0000. */
  function decodeString(key, position) {
    let text = '';
    while (key[position] !== KEY_END) {
      if (key[position] === KEY_ESCAPE) {
        position++;
        text += fromCharCode(key.charCodeAt(position) - 1);
      } else {
        text += key[position];
      }
      position++;
    }
    return [text, position + 1];
  }

  /** Decode a binary body into a new ArrayBuffer. */
  function decodeBinary(key, position) {
    const end = key.indexOf(KEY_END, position);
    const bytes = new NativeUint8Array(end - position);
    for (let index = 0; index < bytes.length; index++) {
      bytes[index] = key.charCodeAt(position + index) - BYTE_SHIFT;
    }
    return [bytes.buffer, end + 1];
  }

  /** Decode array elements up to the end mark. */
  function decodeArray(key, position) {
    const values = [];
    while (key[position] !== KEY_END) {
      const [value, next] = decodeKey(key, position);
      values.push(value);
      position = next;
    }
    return [values, position + 1];
  }

  /** The encoded key for a script argument, throwing DataError when it is not a valid key. */
  function requireKey(input) {
    const key = valueToKey(input);
    if (key === null) {
      throw dataError();
    }
    return key;
  }

  /** "Compare two keys" on encoded keys: -1, 0 or 1. */
  function compareKeys(a, b) {
    return a < b ? -1 : a > b ? 1 : 0;
  }
