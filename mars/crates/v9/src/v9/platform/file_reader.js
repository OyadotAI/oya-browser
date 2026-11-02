/**
 * FileReader, after the File API specification and Blink's FileReaderLoader: a constructor that
 * reads a Blob into a text, data URL, binary string or ArrayBuffer result, and reports the read
 * through loadstart, load, abort, error and loadend events. The bytes are read asynchronously and
 * the outcome is delivered in a queued task, as the spec's read operation requires, so a page
 * that starts a read and attaches a listener in the same turn still hears it. Anti-bot
 * challenges read Blobs this way to fingerprint canvases, so a missing constructor stops them
 * before they can answer. Progress events are not dispatched: no read here reports partial bytes.
 */
(function installFileReader(global) {
  'use strict';
  // Shell contexts have no event targets or blobs, so they do not install the File API.
  if (typeof global.EventTarget !== 'function') return;

  /** readyState values the specification names. */
  const EMPTY = 0;
  const LOADING = 1;
  const DONE = 2;
  /** The MIME type a blob without one reads as in a data URL. */
  const DEFAULT_TYPE = 'application/octet-stream';
  /** Bytes per String.fromCharCode call, so long reads never build one huge argument list. */
  const CHUNK = 0x8000;
  /** The event types a read raises; each gets an `on<type>` handler attribute. */
  const EVENTS = ['loadstart', 'progress', 'load', 'abort', 'error', 'loadend'];

  /** Per-reader state, kept in a WeakMap so page scripts cannot reach it. */
  const stateOf = new WeakMap();
  /** Handler attribute slots by reader, then event type: the function and its one listener. */
  const handlers = new WeakMap();
  /** The EventTarget dispatch captured now, so a page that replaces it cannot change delivery. */
  const dispatch = global.EventTarget.prototype.dispatchEvent;

  /** A DOMException with this name. */
  function domError(name, message) {
    return new global.DOMException(message, name);
  }

  /** The state of a reader, or a TypeError when the receiver is not a FileReader. */
  function stateOfReader(reader, member) {
    const state = stateOf.get(reader);
    if (!state) throw new TypeError(`Failed to read the '${member}' property from 'FileReader': Illegal invocation`);
    return state;
  }

  /** The Blob argument of a read, or the TypeError the IDL conversion raises for anything else. */
  function requireBlob(blob, member) {
    if (!(blob instanceof global.Blob)) {
      throw new TypeError(`Failed to execute '${member}' on 'FileReader': parameter 1 is not of type 'Blob'.`);
    }
    return blob;
  }

  /** A binary string of the bytes, built chunk by chunk so the work stays linear in the input. */
  function binaryString(bytes) {
    const parts = [];
    for (let offset = 0; offset < bytes.length; offset += CHUNK) {
      parts.push(String.fromCharCode.apply(null, bytes.subarray(offset, offset + CHUNK)));
    }
    return parts.join('');
  }

  /** The result a finished read holds for its format. */
  function resultOf(format, bytes, type, encoding) {
    if (format === 'arrayBuffer') return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    if (format === 'binaryString') return binaryString(bytes);
    if (format === 'text') return new global.TextDecoder(encoding === undefined ? 'utf-8' : String(encoding)).decode(bytes);
    return `data:${type || DEFAULT_TYPE};base64,${global.btoa(binaryString(bytes))}`;
  }

  /** Queue `work` as a task: a zero-delay timer, the way the page's task source runs it. */
  function queueTask(work) {
    global.setTimeout(work, 0);
  }

  /** Dispatch one event at a reader; its handler attribute runs as one of its listeners. */
  function fire(reader, type) {
    dispatch.call(reader, new global.Event(type));
  }

  /** Store a handler attribute; the first function assigned adds the one listener that calls it. */
  function setHandler(reader, type, value) {
    let slots = handlers.get(reader);
    if (!slots) {
      slots = Object.create(null);
      handlers.set(reader, slots);
    }
    const slot = slots[type] || (slots[type] = { value: null, listened: false });
    slot.value = typeof value === 'function' ? value : null;
    if (slot.value && !slot.listened) {
      slot.listened = true;
      reader.addEventListener(type, (event) => {
        if (slot.value) slot.value.call(reader, event);
      });
    }
  }

  /** An `on<type>` attribute on the prototype, reading and writing its slot. */
  function eventHandler(proto, type) {
    const name = 'on' + type;
    Object.defineProperty(proto, name, {
      enumerable: true,
      configurable: true,
      get: { [name]() { const slot = (handlers.get(this) || {})[type]; return slot ? slot.value : null; } }[name],
      set: { [name](value) { setHandler(this, type, value); } }[name],
    });
  }

  /** Finish a read that was not aborted: the result is stored, then load and loadend are raised. */
  function complete(reader, state, generation, format, bytes) {
    if (generation !== state.generation) return;
    state.readyState = DONE;
    state.result = resultOf(format, bytes, state.blob.type, state.encoding);
    fire(reader, 'load');
    fire(reader, 'loadend');
  }

  /** Start a read: the state goes LOADING, loadstart is queued and the bytes are read in a task. */
  function startRead(reader, member, format, blob, encoding) {
    const state = stateOfReader(reader, member);
    if (state.readyState === LOADING) {
      throw domError('InvalidStateError', "Failed to execute '" + member + "' on 'FileReader': The object is already busy reading Blobs.");
    }
    state.readyState = LOADING;
    state.result = null;
    state.blob = blob;
    state.encoding = encoding;
    const generation = ++state.generation;
    queueTask(() => fire(reader, 'loadstart'));
    blob.arrayBuffer().then((buffer) => {
      queueTask(() => complete(reader, state, generation, format, new Uint8Array(buffer)));
    });
  }

  /** The FileReader interface. Its state lives beside it, so a read in progress is never visible as a plain property. */
  class FileReader extends global.EventTarget {
    constructor() {
      super();
      stateOf.set(this, { readyState: EMPTY, result: null, blob: null, encoding: undefined, generation: 0 });
    }
    get readyState() { return stateOfReader(this, 'readyState').readyState; }
    get result() { return stateOfReader(this, 'result').result; }
    get error() { stateOfReader(this, 'error'); return null; }
    readAsArrayBuffer(blob) { startRead(this, 'readAsArrayBuffer', 'arrayBuffer', requireBlob(blob, 'readAsArrayBuffer')); }
    readAsBinaryString(blob) { startRead(this, 'readAsBinaryString', 'binaryString', requireBlob(blob, 'readAsBinaryString')); }
    readAsText(blob, encoding) { startRead(this, 'readAsText', 'text', requireBlob(blob, 'readAsText'), encoding); }
    readAsDataURL(blob) { startRead(this, 'readAsDataURL', 'dataURL', requireBlob(blob, 'readAsDataURL')); }
    abort() {
      const state = stateOfReader(this, 'abort');
      state.result = null;
      if (state.readyState !== LOADING) return;
      state.readyState = DONE;
      state.generation++;
      fire(this, 'abort');
      fire(this, 'loadend');
    }
  }
  for (const type of EVENTS) eventHandler(FileReader.prototype, type);
  for (const target of [FileReader, FileReader.prototype]) {
    Object.defineProperty(target, 'EMPTY', { value: EMPTY, enumerable: true });
    Object.defineProperty(target, 'LOADING', { value: LOADING, enumerable: true });
    Object.defineProperty(target, 'DONE', { value: DONE, enumerable: true });
  }
  Object.defineProperty(global, 'FileReader', { value: FileReader, writable: true, enumerable: false, configurable: true });
})(globalThis);
