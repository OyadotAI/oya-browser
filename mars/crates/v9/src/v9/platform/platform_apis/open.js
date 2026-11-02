/**
 * Platform APIs whose work happens in the coordinator: service workers, Cache Storage, the
 * clipboard, Web Locks, the Cookie Store, Storage Access, credentials, Screen Wake Lock,
 * gamepads, Web MIDI, Encrypted Media Extensions (Clear Key) and installed related apps.
 * The files in `platform_apis/` are concatenated between this opening and `close.js` into one
 * closure, so their shared helpers stay private; `close.js` installs them lazily. The closure takes the native bridge
 * (`__marsPlatformHost`) and deletes the global before any page script runs; every request
 * then crosses as one JSON "platform" HostRequest whose answer is `{ok}` or `{error:{name,message}}`.
 */
(function installPlatformApis(global) {
  'use strict';
  const host = global.__marsPlatformHost;
  // The bridge is an interface global, so it lives on an object in the global's prototype chain.
  for (let holder = global; holder; holder = Object.getPrototypeOf(holder)) {
    if (Object.prototype.hasOwnProperty.call(holder, '__marsPlatformHost')) delete holder.__marsPlatformHost;
  }
  if (!host) return;
  /** Which global this is: "window", "frame", "worker" or "service". */
  const CONTEXT = host.context();
  /** Captured now, so a page that replaces them cannot change what crosses the bridge. */
  const toJson = JSON.stringify;
  const fromJson = JSON.parse;
  const defineProperty = Object.defineProperty;
  const create = Object.create;
  const freeze = Object.freeze;
  const isArray = Array.isArray;
  const DOMException = global.DOMException;
  const EventCtor = global.Event;
  /** Whether `installAll` has run in this context. */
  let installed = false;

  /**
   * Install every API. It runs the first time a page touches any of them (the triggers in
   * `close.js`), not at bootstrap, so a page that never uses them carries none of their
   * closures, prototypes or state in its heap and in every serialized step.
   */
  function installAll() {
  if (installed) return;
  installed = true;

  /** A DOMException with this name. */
  function domError(name, message) {
    return new DOMException(message, name);
  }

  /** The error an `{error}` answer describes: a TypeError for "TypeError", else a DOMException. */
  function answerError(error) {
    if (error.name === 'TypeError') return new TypeError(error.message);
    return domError(error.name, error.message);
  }

  /** Send one request; resolves with the answer's `ok` value or rejects with its error. */
  async function call(op, args, bytes) {
    const text = await host.call(op, toJson(args === undefined ? null : args), bytes);
    const answer = fromJson(text);
    if (answer.error) throw answerError(answer.error);
    return answer.ok;
  }

  /** `call` for an answer that may carry bytes: resolves `{ok, bytes}` (bytes an ArrayBuffer or null). */
  async function callBytes(op, args, bytes) {
    const result = await host.call(op, toJson(args === undefined ? null : args), bytes);
    const text = isArray(result) ? result[0] : result;
    const answer = fromJson(text);
    if (answer.error) throw answerError(answer.error);
    return { ok: answer.ok, bytes: isArray(result) ? result[1] : null };
  }

  /** Whether the page holds transient user activation now. */
  function isActive() {
    return host.activation()[1];
  }

  /** Define a non-enumerable, configurable, writable value as WebIDL operations are. */
  function method(target, name, value) {
    defineProperty(target, name, { value, writable: true, enumerable: true, configurable: true });
  }

  /** Install `methods` on `proto`. */
  function methods(proto, table) {
    if (!proto) return;
    for (const name of Object.keys(table)) method(proto, name, table[name]);
  }

  /** A read-only accessor on `proto` reading `read(this)`. */
  function getter(proto, name, read) {
    if (!proto) return;
    defineProperty(proto, name, { get: { [name]() { return read(this); } }[name], enumerable: true, configurable: true });
  }

  /** Replace or define a global interface object (non-enumerable, as WebIDL interfaces are). */
  function defineGlobal(name, value) {
    defineProperty(global, name, { value, writable: true, enumerable: false, configurable: true });
  }

  /** The prototype of a global interface, or a fresh object inheriting `parent` when absent. */
  function prototypeOf(name, parent) {
    const ctor = global[name];
    if (ctor && ctor.prototype) return ctor.prototype;
    return create(parent || Object.prototype);
  }

  /** An instance of interface `name` (its existing prototype) carrying no own properties. */
  function instance(name, parent) {
    return create(prototypeOf(name, parent));
  }

  /** Handler slots by target, then event name. */
  const handlers = new WeakMap();

  /** An `on<type>` attribute on `proto`: assigning a function makes it a listener for `type`. */
  function eventHandler(proto, type) {
    if (!proto) return;
    const name = 'on' + type;
    defineProperty(proto, name, {
      enumerable: true,
      configurable: true,
      get: { [name]() { const slots = handlers.get(this); return (slots && slots[type] && slots[type].value) || null; } }[name],
      set: { [name](value) { setHandler(this, type, value); } }[name],
    });
  }

  /** Store a handler, adding one listener that calls whichever handler is current. */
  function setHandler(target, type, value) {
    let slots = handlers.get(target);
    if (!slots) handlers.set(target, (slots = create(null)));
    const slot = slots[type] || (slots[type] = { value: null, listener: null });
    slot.value = typeof value === 'function' || (typeof value === 'object' && value !== null) ? value : null;
    if (slot.value && !slot.listener) {
      slot.listener = function (event) {
        const handler = slot.value;
        if (typeof handler !== 'function') return undefined;
        const result = handler.call(this, event);
        if (result === false) event.preventDefault();
        return result;
      };
      target.addEventListener(type, slot.listener);
    }
  }

  /** An Event of `type` with extra read-only fields, shaped as interface `name` when it exists. */
  function makeEvent(name, type, init, fields) {
    const event = new EventCtor(type, init || {});
    const ctor = global[name];
    if (ctor && ctor.prototype && ctor.prototype !== EventCtor.prototype) {
      Object.setPrototypeOf(event, ctor.prototype);
    }
    for (const key of Object.keys(fields || {})) {
      defineProperty(event, key, { value: fields[key], enumerable: true, configurable: true });
    }
    return event;
  }

  /** A constructible Event subclass `name` whose init dictionary members become fields. */
  function eventClass(name, members) {
    const ctor = {
      [name]: function (type, init) {
        if (!new.target) throw new TypeError(`Failed to construct '${name}': Please use the 'new' operator.`);
        if (arguments.length < 1) throw new TypeError(`Failed to construct '${name}': 1 argument required, but only 0 present.`);
        const fields = {};
        for (const key of Object.keys(members)) {
          fields[key] = init && init[key] !== undefined ? init[key] : members[key];
        }
        return makeEvent(name, String(type), init, fields);
      },
    }[name];
    const old = global[name];
    ctor.prototype = old && old.prototype ? old.prototype : create(EventCtor.prototype);
    defineProperty(ctor.prototype, 'constructor', { value: ctor, writable: true, configurable: true });
    Object.setPrototypeOf(ctor, EventCtor);
    defineGlobal(name, ctor);
    return ctor;
  }

  /** Event topics the coordinator queues for this context, and their consumers. */
  const topics = create(null);
  /** Whether an `events.next` request is outstanding. */
  let pumping = false;

  /** Subscribe to coordinator events of `topic`; `consume(event)` receives each one. */
  function subscribe(topic, consume) {
    if (topics[topic]) return topics[topic].ready;
    const entry = { consume, ready: call('events.subscribe', { topic }) };
    topics[topic] = entry;
    entry.ready.then(pump, () => {});
    return entry.ready;
  }

  /** Keep one `events.next` outstanding while anything is subscribed; route each event. */
  function pump() {
    if (pumping) return;
    pumping = true;
    call('events.next', {}).then((events) => {
      pumping = false;
      for (const event of events || []) {
        const entry = topics[event.topic];
        if (entry) {
          try { entry.consume(event); } catch (error) { reportError(error); }
        }
      }
      pump();
    }, () => { pumping = false; });
  }

  /** Report an exception thrown by a page callback to the global, as a task would. */
  function reportError(error) {
    if (typeof global.reportError === 'function') global.reportError(error);
    else Promise.reject(error);
  }

  /** Base64 of an ArrayBuffer or view, for JSON transport of small binary fields. */
  function toBase64(buffer) {
    const bytes = buffer instanceof Uint8Array ? buffer : new Uint8Array(ArrayBuffer.isView(buffer) ? buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength) : buffer);
    let text = '';
    for (let i = 0; i < bytes.length; i += 1) text += String.fromCharCode(bytes[i]);
    return global.btoa(text);
  }

  /** The bytes of base64 `text` as a Uint8Array. */
  function fromBase64(text) {
    const raw = global.atob(text);
    const bytes = new Uint8Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) bytes[i] = raw.charCodeAt(i);
    return bytes;
  }

  /** Base64url without padding, as JWK and Clear Key messages use. */
  function toBase64Url(bytes) {
    return toBase64(bytes).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }

  /** Bytes of base64url `text`, or null when it is not base64url. */
  function fromBase64Url(text) {
    if (typeof text !== 'string' || !/^[A-Za-z0-9_-]*$/.test(text)) return null;
    const padded = text.replace(/-/g, '+').replace(/_/g, '/') + '==='.slice((text.length + 3) % 4);
    try { return fromBase64(padded); } catch (error) { return null; }
  }

  /** A BufferSource's bytes as a fresh Uint8Array; TypeError for anything else. */
  function bufferBytes(value, what) {
    if (value instanceof ArrayBuffer) return new Uint8Array(value.slice(0));
    if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer.slice(value.byteOffset, value.byteOffset + value.byteLength));
    throw new TypeError(`${what} is not of type '(ArrayBuffer or ArrayBufferView)'.`);
  }

  /** An AbortSignal's abort reason as an error, or null when not aborted or no signal. */
  function abortReason(signal) {
    if (!signal || !signal.aborted) return null;
    return signal.reason !== undefined ? signal.reason : domError('AbortError', 'The operation was aborted.');
  }

  /** Whether this context is a secure context, as these APIs require. */
  function secure() {
    return global.isSecureContext !== false;
  }
