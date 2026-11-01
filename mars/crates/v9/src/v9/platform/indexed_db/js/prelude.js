/**
 * IndexedDB, as the W3C Indexed Database API 3.0 specifies it, for one page or dedicated worker.
 * The engine runs this once for each fresh context, before any page script, with `host`: the
 * native seam that dispatches trusted events through the request -> transaction -> database
 * path, loads and commits database images through the coordinator, reads Blob bytes and runs a
 * microtask checkpoint. Everything else (key comparison, ordered stores and indexes, cursors,
 * transactions and their scheduling) lives in this closure, because the specification is
 * written as tasks, events and promises the engine already implements. The sections that follow
 * are concatenated into this one function; only the interfaces installed at the end are visible.
 */
(function installIndexedDB(global, host) {
  'use strict';

  /** Primordials captured now, so page code that replaces them cannot change IndexedDB. */
  const ObjectCreate = Object.create;
  const defineProperty = Object.defineProperty;
  const getPrototypeOf = Object.getPrototypeOf;
  const setPrototypeOf = Object.setPrototypeOf;
  const getOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
  const objectKeys = Object.keys;
  const hasOwn = Object.prototype.hasOwnProperty;
  const apply = Reflect.apply;
  const construct = Reflect.construct;
  const isArray = Array.isArray;
  const isView = ArrayBuffer.isView;
  const fromCharCode = String.fromCharCode;
  const jsonParse = JSON.parse;
  const jsonStringify = JSON.stringify;
  const mathFloor = Math.floor;
  const mathMin = Math.min;
  const NativeWeakMap = WeakMap;
  const NativeMap = Map;
  const NativeSet = Set;
  const NativePromise = Promise;
  const NativeTypeError = TypeError;
  const NativeDOMException = global.DOMException;
  const NativeEvent = global.Event;
  const NativeEventTarget = global.EventTarget;
  const NativeUint8Array = Uint8Array;
  const NativeDataView = DataView;
  const NativeArrayBuffer = ArrayBuffer;
  const NativeDate = Date;
  const NativeBlob = global.Blob;
  const NativeFile = global.File;
  const structuredCloneNative = global.structuredClone;
  const setTimeoutNative = global.setTimeout;
  const btoaNative = global.btoa;
  const atobNative = global.atob;
  const addListener = NativeEventTarget.prototype.addEventListener;
  const dateGetTime = NativeDate.prototype.getTime;
  const bufferByteLength = getOwnPropertyDescriptor(NativeArrayBuffer.prototype, 'byteLength').get;
  const viewBuffer = getOwnPropertyDescriptor(getPrototypeOf(NativeUint8Array.prototype), 'buffer').get;
  const viewByteOffset = getOwnPropertyDescriptor(getPrototypeOf(NativeUint8Array.prototype), 'byteOffset').get;
  const typedByteLength = getOwnPropertyDescriptor(getPrototypeOf(NativeUint8Array.prototype), 'byteLength').get;
  const typedTag = getOwnPropertyDescriptor(getPrototypeOf(NativeUint8Array.prototype), Symbol.toStringTag).get;
  const dataViewBuffer = getOwnPropertyDescriptor(NativeDataView.prototype, 'buffer').get;
  const dataViewOffset = getOwnPropertyDescriptor(NativeDataView.prototype, 'byteOffset').get;
  const dataViewLength = getOwnPropertyDescriptor(NativeDataView.prototype, 'byteLength').get;
  const getFloat64 = NativeDataView.prototype.getFloat64;
  const setFloat64 = NativeDataView.prototype.setFloat64;
  const getUint32 = NativeDataView.prototype.getUint32;
  const setUint32 = NativeDataView.prototype.setUint32;
  const mapGet = NativeMap.prototype.get;
  const setHas = NativeSet.prototype.has;
  const regexpSource = getOwnPropertyDescriptor(RegExp.prototype, 'source').get;
  const regexpFlags = getOwnPropertyDescriptor(RegExp.prototype, 'flags').get;

  /** The largest integer a key generator or version may hold (2^53 - 1), as WebIDL's [EnforceRange] unsigned long long admits. */
  const MAX_SAFE = 9007199254740991;
  /** The key generator's ceiling: past 2^53 the generator is exhausted and adds fail with ConstraintError. */
  const GENERATOR_LIMIT = 9007199254740992;
  /** Requests one scheduler turn may run before yielding back to the event loop. */
  const TURN_REQUESTS = 512;
  /** Bytes one database may hold, measured as its stored keys and serialized values; past it a commit fails with QuotaExceededError. */
  const DATABASE_BYTES = 4 * 1024 * 1024;
  /** The flag `host.dispatch` returns when a listener canceled the event. */
  const DISPATCH_CANCELED = 1;
  /** The flag `host.dispatch` returns when a listener threw. */
  const DISPATCH_THREW = 2;
  /** The origin this context's storage belongs to; an opaque origin has no IndexedDB. */
  const ORIGIN = global.location ? String(global.location.origin) : 'null';

  /** Each interface object's internal state, keyed by the object scripts see. */
  const internals = new NativeWeakMap();
  /** WeakMap accessors captured for `internals`. */
  const weakGet = NativeWeakMap.prototype.get;
  const weakSet = NativeWeakMap.prototype.set;

  /** Throw a TypeError unless `object` is an instance of `brand` built by this file; returns its state. */
  function stateOf(object, brand) {
    const state = typeof object === 'object' && object !== null ? apply(weakGet, internals, [object]) : undefined;
    if (!state || state.brand !== brand) {
      throw new NativeTypeError('Illegal invocation');
    }
    return state;
  }
  /** Attach internal state to a new interface object. */
  function bind(object, state) {
    apply(weakSet, internals, [object, state]);
    return object;
  }

  /** A DOMException with the given name, as every IndexedDB failure reports itself. */
  function domError(name, message) {
    return new NativeDOMException(message, name);
  }

  /** Shorthand for the TransactionInactiveError every operation on an inactive transaction throws. */
  function inactiveError() {
    return domError('TransactionInactiveError', 'The transaction is not active.');
  }

  /** Shorthand for the InvalidStateError thrown on deleted stores, finished transactions and pending requests. */
  function stateError(message) {
    return domError('InvalidStateError', message);
  }

  /** Shorthand for the DataError thrown for values that are not valid keys or ranges. */
  function dataError(message) {
    return domError('DataError', message || 'The parameter is not a valid key.');
  }

  /** Call `fn` on `self` through the captured Reflect.apply. */
  function call(fn, self, ...args) {
    return apply(fn, self, args);
  }

  /** Whether a value is an object or a function. */
  function isObject(value) {
    return (typeof value === 'object' && value !== null) || typeof value === 'function';
  }

  /** Whether `object` has `name` as an own property, through the captured hasOwnProperty. */
  function own(object, name) {
    return call(hasOwn, object, name);
  }
