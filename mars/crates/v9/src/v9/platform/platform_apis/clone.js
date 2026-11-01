  /**
   * Structured data for messages between a page and its service worker, which run in different
   * engine contexts: JSON with tagged records for the structured-clone types JSON cannot carry
   * (undefined, Date, RegExp, Map, Set, ArrayBuffer, typed arrays, BigInt, non-finite numbers
   * and negative zero), cycles and shared references. Functions and platform objects throw
   * DataCloneError, as structuredClone does.
   */
  function encodeClone(value) {
    const seen = new Map();
    const records = [];
    /** One value: primitives inline, objects as references into `records`. */
    function encode(item) {
      if (item === undefined) return { u: 1 };
      if (typeof item === 'number') return Number.isFinite(item) && !Object.is(item, -0) ? item : { n: Object.is(item, -0) ? '-0' : String(item) };
      if (typeof item === 'bigint') return { b: String(item) };
      if (item === null || typeof item === 'string' || typeof item === 'boolean') return item;
      if (typeof item !== 'object') throw domError('DataCloneError', `${String(item)} could not be cloned.`);
      if (seen.has(item)) return { r: seen.get(item) };
      const index = records.length;
      seen.set(item, index);
      records.push(null);
      records[index] = encodeObject(item, encode);
      return { r: index };
    }
    return { root: encode(value), records };
  }

  /** The record for one object: its kind tag and encoded contents. */
  function encodeObject(item, encode) {
    if (item instanceof Date) return { t: 'Date', v: item.getTime() };
    if (item instanceof RegExp) return { t: 'RegExp', v: [item.source, item.flags] };
    if (item instanceof ArrayBuffer) return { t: 'ArrayBuffer', v: toBase64(item) };
    if (ArrayBuffer.isView(item)) return { t: item.constructor.name, v: toBase64(item) };
    if (item instanceof Map) return { t: 'Map', v: [...item].map(([key, entry]) => [encode(key), encode(entry)]) };
    if (item instanceof Set) return { t: 'Set', v: [...item].map(encode) };
    if (item instanceof Error) return { t: 'Error', v: [item.name, item.message] };
    if (isArray(item)) return { t: 'Array', v: item.map(encode) };
    const proto = Object.getPrototypeOf(item);
    if (proto !== Object.prototype && proto !== null) {
      throw domError('DataCloneError', `${Object.prototype.toString.call(item)} could not be cloned.`);
    }
    return { t: 'Object', v: Object.keys(item).map((key) => [key, encode(item[key])]) };
  }

  /** Rebuild a value `encodeClone` produced. */
  function decodeClone(data) {
    if (!data || typeof data !== 'object' || !isArray(data.records)) return undefined;
    const built = [];
    /** One encoded value. */
    function decode(item) {
      if (item === null || typeof item !== 'object') return item;
      if (item.u) return undefined;
      if (item.n !== undefined) return item.n === '-0' ? -0 : Number(item.n);
      if (item.b !== undefined) return BigInt(item.b);
      if (built[item.r] !== undefined) return built[item.r];
      return decodeRecord(data.records[item.r], item.r);
    }
    /** One object record, registered before its contents so cycles resolve. */
    function decodeRecord(record, index) {
      const kind = record.t;
      const value = record.v;
      if (kind === 'Array') { const out = []; built[index] = out; value.forEach((entry) => out.push(decode(entry))); return out; }
      if (kind === 'Object') { const out = {}; built[index] = out; for (const [key, entry] of value) out[key] = decode(entry); return out; }
      if (kind === 'Map') { const out = new Map(); built[index] = out; for (const [key, entry] of value) out.set(decode(key), decode(entry)); return out; }
      if (kind === 'Set') { const out = new Set(); built[index] = out; for (const entry of value) out.add(decode(entry)); return out; }
      let out;
      if (kind === 'Date') out = new Date(value);
      else if (kind === 'RegExp') out = new RegExp(value[0], value[1]);
      else if (kind === 'ArrayBuffer') out = fromBase64(value).buffer;
      else if (kind === 'Error') { out = new Error(value[1]); out.name = value[0]; }
      else if (/^(Ui|I)nt(8|16|32)Array$|^Uint8ClampedArray$|^Float(32|64)Array$|^Big(Ui|I)nt64Array$|^DataView$/.test(kind)) out = new global[kind](fromBase64(value).buffer);
      built[index] = out;
      return out;
    }
    return decode(data.root);
  }
