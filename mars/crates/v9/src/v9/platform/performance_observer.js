/**
 * PerformanceObserver and PerformanceObserverEntryList over the page's real performance timeline
 * (marks, measures and the navigation entry). Chrome's shapes made the interface exist with an
 * illegal constructor and no `supportedEntryTypes`, so bing.com's
 * `PerformanceObserver.supportedEntryTypes.indexOf(...)` threw on load. Observers are notified
 * when performance.mark and performance.measure record an entry, and receive their records in a
 * queued task, as the Performance Timeline specification queues the observer callback.
 */
(function installPerformanceObserver(global) {
  'use strict';
  const perf = global.performance;
  if (!perf || typeof perf.getEntriesByType !== 'function') return;
  /** The entry types this timeline actually records. */
  const SUPPORTED = Object.freeze(['mark', 'measure', 'navigation']);
  const setTimer = global.setTimeout;
  /** Observer to its callback, observed types, buffer and delivery flag. */
  const stateOf = new WeakMap();
  /** Registered observers, in registration order. */
  const registered = new Set();

  /** Entries sorted by start time, as entry lists hand them out. */
  function sorted(entries) {
    return entries.slice().sort((a, b) => a.startTime - b.startTime);
  }

  class PerformanceObserverEntryList {
    constructor(entries) {
      stateOf.set(this, sorted(entries));
    }
    getEntries() { return stateOf.get(this).slice(); }
    getEntriesByType(type) { return stateOf.get(this).filter((e) => e.entryType === String(type)); }
    getEntriesByName(name, type) {
      return stateOf.get(this).filter((e) => e.name === String(name) && (type === undefined || e.entryType === String(type)));
    }
  }

  /** Queue the observer's callback once for everything buffered so far. */
  function schedule(observer, state) {
    if (state.queued) return;
    state.queued = true;
    setTimer(() => {
      state.queued = false;
      if (!registered.has(observer) || state.buffer.length === 0) return;
      const records = state.buffer.splice(0);
      try {
        state.callback.call(observer, new PerformanceObserverEntryList(records), observer, { droppedEntriesCount: 0 });
      } catch (error) {
        if (typeof global.reportError === 'function') global.reportError(error); else setTimer(() => { throw error; }, 0);
      }
    }, 0);
  }

  /** Offer a new entry to every observer watching its type. */
  function notify(entry) {
    if (!entry) return;
    for (const observer of registered) {
      const state = stateOf.get(observer);
      if (state.types.has(entry.entryType)) {
        state.buffer.push(entry);
        schedule(observer, state);
      }
    }
  }

  /** The types an observe() call names, validated as the specification requires. */
  function requestedTypes(options) {
    const hasList = options.entryTypes !== undefined;
    const hasType = options.type !== undefined;
    if (!hasList && !hasType) throw new TypeError("Failed to execute 'observe' on 'PerformanceObserver': An observe() call must include either entryTypes or type.");
    if (hasList && (hasType || options.buffered !== undefined)) {
      throw new TypeError("Failed to execute 'observe' on 'PerformanceObserver': An observe() call must not include both entryTypes and type or buffered.");
    }
    const names = hasList ? Array.from(options.entryTypes, String) : [String(options.type)];
    return { names: names.filter((name) => SUPPORTED.includes(name)), single: !hasList };
  }

  class PerformanceObserver {
    constructor(callback) {
      if (typeof callback !== 'function') {
        throw new TypeError("Failed to construct 'PerformanceObserver': The callback provided as parameter 1 is not a function.");
      }
      stateOf.set(this, { callback, types: new Set(), buffer: [], queued: false });
    }
    observe(options = {}) {
      const state = stateOf.get(this);
      const { names, single } = requestedTypes(options);
      if (names.length === 0) return;
      if (!single) state.types.clear();
      for (const name of names) state.types.add(name);
      registered.add(this);
      if (single && options.buffered) {
        state.buffer.push(...perf.getEntriesByType(names[0]));
        if (state.buffer.length) schedule(this, state);
      }
    }
    disconnect() {
      const state = stateOf.get(this);
      registered.delete(this);
      state.buffer.length = 0;
      state.types.clear();
    }
    takeRecords() {
      return stateOf.get(this).buffer.splice(0);
    }
    static get supportedEntryTypes() { return SUPPORTED; }
  }

  /** Wrap a recording method, on whichever object in the chain defines it, so observers hear of
  each entry it returns. */
  function observeRecording(start, name) {
    let proto = start;
    while (proto && !Object.prototype.hasOwnProperty.call(proto, name)) proto = Object.getPrototypeOf(proto);
    if (!proto) return;
    const original = proto[name];
    if (typeof original !== 'function') return;
    const wrapped = { [name](...args) { const entry = original.apply(this, args); notify(entry); return entry; } }[name];
    Object.defineProperty(proto, name, { value: wrapped, writable: true, enumerable: true, configurable: true });
  }
  observeRecording(perf, 'mark');
  observeRecording(perf, 'measure');

  function define(target, name, value) {
    Object.defineProperty(target, name, { value, writable: true, enumerable: false, configurable: true });
  }
  define(global, 'PerformanceObserver', PerformanceObserver);
  define(global, 'PerformanceObserverEntryList', PerformanceObserverEntryList);
})(globalThis);
