/**
 * Anti-detection stealth, builds a JS string injected before page code runs.
 *
 * Two rules, both learned from CreepJS's lie battery:
 *
 * 1. Prefer native. Anything CDP emulation can set (webdriver, platform,
 *    hardwareConcurrency, locale, timezone, screen) is set there by apply.js,
 *    and the patches below only fire where the running value is still wrong.
 *    A value Chrome reports itself can never be caught lying.
 * 2. Where a patch is needed, it must be shaped like the native it replaces:
 *    not constructible, no `prototype`, own keys exactly length+name, the right
 *    name and length, "[native code]" from any realm's toString, and getters
 *    that throw Illegal invocation when read off the prototype.
 */

const { randomBytes } = require('crypto');

/**
 * The mask and the helpers every patch must use. Emitted once at the top of
 * the combined injection so every patch after it is covered. Works in page and
 * worker scopes alike.
 */
function buildMaskPreamble() {
  // One key per build. Every frame of a target gets the same source, so
  // same-origin frames can find each other's registry; nothing else can.
  const key = randomBytes(12).toString('hex');
  return `
  // ── Captured before any page script runs ──
  // A page that later patches Reflect, WeakMap or Object can neither watch
  // the mask work nor break it.
  const _apply = Reflect.apply;
  const _defProp = Object.defineProperty;
  const _getDesc = Object.getOwnPropertyDescriptor;
  const _wmGet = WeakMap.prototype.get;
  const _wmSet = WeakMap.prototype.set;
  const _origToString = Function.prototype.toString;
  const _global = typeof window !== 'undefined' ? window : self;

  // ── Native toString mask ──
  // One registry for every same-origin realm. Detectors call ANOTHER frame's
  // Function.prototype.toString on this frame's functions (CreepJS runs its
  // lie battery from a "phantom" iframe), so a frame adopts its parent's
  // registry through a key only this injection knows.
  const _KEY = '${key}';
  let _native = null;
  try {
    const parent = _global.parent;
    if (parent && parent !== _global) {
      const shared = _apply(parent.Function.prototype.toString, null, [_KEY]);
      // The registry may come from the top frame, a realm neither this one nor
      // the parent owns, so no instanceof: WeakMap.prototype.get accepts a
      // WeakMap from any realm and throws for anything else.
      _apply(_wmGet, shared, [_global]);
      _native = shared;
    }
  } catch {}
  if (!_native) _native = new WeakMap();

  const _mark = (fn, name) => {
    try { _apply(_wmSet, _native, [fn, name === undefined ? fn.name : name]); } catch {}
    return fn;
  };

  // A method, not a Proxy and not a function declaration: a Proxy fails
  // CreepJS's prototype-cycle checks ("too much recursion", "reflect set
  // proto"), and a declaration is constructible and carries a prototype.
  const _toString = { toString() {
    if (arguments.length === 1 && arguments[0] === _KEY) return _native;
    const name = _apply(_wmGet, _native, [this]);
    if (name !== undefined) return 'function ' + name + '() { [native code] }';
    return _apply(_origToString, this, arguments);
  } }.toString;
  _mark(_toString, 'toString');
  try { Function.prototype.toString = _toString; } catch {}

  /** A function shaped like a native method: own keys length+name only. */
  const _nativeLike = (name, impl, length) => {
    const fn = { [name](...args) { return _apply(impl, this, args); } }[name];
    try { _defProp(fn, 'length', { value: length }); } catch {}
    return _mark(fn, name);
  };

  /** Replace a method in place (its descriptor is kept). Returns the original. */
  const _patch = (target, name, make) => {
    try {
      const orig = target[name];
      if (typeof orig !== 'function') return null;
      target[name] = _nativeLike(name, make(orig), orig.length);
      return orig;
    } catch { return null; }
  };

  /**
   * A read-only accessor shaped like a native one. Reading it off the
   * prototype throws, as the real one does; returning a value there is
   * CreepJS's "descriptor.value undefined" lie.
   */
  const _defineGetter = (target, prop, value) => {
    const Brand = typeof target.constructor === 'function' && target.constructor.prototype === target
      ? target.constructor : null;
    const get = _getDesc({ get [prop]() {
      if (Brand && !(this instanceof Brand)) throw new TypeError('Illegal invocation');
      return value;
    } }, prop).get;
    _mark(get, 'get ' + prop);
    try { _defProp(target, prop, { get, set: undefined, enumerable: true, configurable: true }); } catch {}
  };

  /** Define only where the running value is wrong. Emulated values need no lie. */
  const _ensure = (target, instance, prop, want) => {
    let now;
    try { now = instance[prop]; } catch {}
    if (now !== want) _defineGetter(target, prop, want);
  };

  /**
   * Deterministic noise: a pure function of the seed and the inputs, so the
   * same query returns the same answer, as a real browser does.
   */
  const _noise = (seed, ...parts) => {
    let h = (seed >>> 0) || 1;
    for (const part of parts) {
      const str = String(part);
      for (let i = 0; i < str.length; i++) {
        h = (Math.imul(h, 31) + str.charCodeAt(i)) >>> 0;
      }
      h = (h ^ (h >>> 13)) >>> 0;
    }
    return ((h >>> 8) / 0x1000000) - 0.5;   // [-0.5, 0.5)
  };
`;
}

/** The stealth patches themselves. Page scope; assumes the mask preamble. */
function buildStealthBody() {
  return `
  // ── navigator.webdriver ──
  // Emulation.setAutomationOverride makes this natively false. The getter is
  // for runtimes where that did not apply; on the prototype, never the
  // instance, and false rather than undefined, undefined is itself the tell.
  if (navigator.webdriver !== false) _defineGetter(Navigator.prototype, 'webdriver', false);

  // ── Remove Electron globals ──
  // Tabs run with contextIsolation and sandbox, so these should already be
  // absent; harmless belt-and-braces for any surface that is not.
  try { delete window.process; } catch {}
  try { delete window.require; } catch {}
  try { delete window.module; } catch {}
  try { delete window.exports; } catch {}
  try { delete window.__dirname; } catch {}
  try { delete window.__filename; } catch {}

  // ── window.chrome ──
  // Filled in only where missing: Chrome has its own, and replacing a native
  // member is a lie. chrome.runtime is deliberately never defined, it exists
  // only on extension pages, so defining it is evidence, not cover.
  if (!window.chrome) window.chrome = {};
  if (!window.chrome.app) {
    window.chrome.app = {
      isInstalled: false,
      InstallState: { DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' },
      RunningState: { CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' },
      getDetails: _nativeLike('getDetails', () => null, 0),
      getIsInstalled: _nativeLike('getIsInstalled', () => false, 0),
      installState: _nativeLike('installState', (cb) => { if (cb) cb('not_installed'); }, 0),
    };
  }
  if (!window.chrome.csi) {
    window.chrome.csi = _nativeLike('csi', () => ({ onloadT: Date.now(), startE: Date.now(), pageT: performance.now() }), 0);
  }
  if (!window.chrome.loadTimes) {
    window.chrome.loadTimes = _nativeLike('loadTimes', () => {
      const nav = performance.getEntriesByType('navigation')[0] || {};
      const origin = performance.timeOrigin / 1000;
      return {
        commitLoadTime: origin + (nav.responseStart || 0) / 1000,
        connectionInfo: 'h2',
        finishDocumentLoadTime: origin + (nav.domContentLoadedEventEnd || 0) / 1000,
        finishLoadTime: origin + (nav.loadEventEnd || 0) / 1000,
        firstPaintAfterLoadTime: 0,
        firstPaintTime: origin + (nav.responseEnd || 0) / 1000,
        navigationType: 'Other',
        npnNegotiatedProtocol: 'h2',
        requestTime: origin + (nav.startTime || 0) / 1000,
        startLoadTime: origin + (nav.startTime || 0) / 1000,
        wasAlternateProtocolAvailable: false,
        wasFetchedViaSpdy: true,
        wasNpnNegotiated: true,
      };
    }, 0);
  }

  // ── navigator.plugins / mimeTypes ──
  // Only where the runtime ships none (Electron). Modern Chrome has these
  // natively, and a native list replaced by ours is a lie. The types matter as
  // much as the data: "[object PluginArray]", not "[object Object]".
  const _pluginSpecs = [
    { name: 'PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'Chrome PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'Chromium PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'Microsoft Edge PDF Viewer', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
    { name: 'WebKit built-in PDF', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
  ];
  const _mimeSpecs = [
    { type: 'application/pdf', suffixes: 'pdf', description: 'Portable Document Format' },
    { type: 'text/pdf', suffixes: 'pdf', description: 'Portable Document Format' },
  ];

  /**
   * Borrow the real interface prototype so instanceof and Symbol.toStringTag
   * are correct. If the interface is missing we leave the surface alone
   * rather than install something that reads as a lie.
   */
  const _asInterface = (obj, Ctor) => {
    if (typeof Ctor !== 'function' || !Ctor.prototype) return null;
    try { Object.setPrototypeOf(obj, Ctor.prototype); return obj; } catch { return null; }
  };

  const _buildArrayLike = (items, Ctor, namedKey) => {
    const arr = Object.create(null);
    const list = items.slice();
    list.forEach((item, i) => { arr[i] = item; });
    Object.defineProperty(arr, 'length', { value: list.length, enumerable: false });
    arr.item = _nativeLike('item', (i) => list[i] || null, 1);
    arr.namedItem = _nativeLike('namedItem', (n) => list.find((x) => x[namedKey] === n) || null, 1);
    if (Ctor === window.PluginArray) arr.refresh = _nativeLike('refresh', () => {}, 0);
    list.forEach((item) => { arr[item[namedKey]] = item; });
    return _asInterface(arr, Ctor) || arr;
  };

  if (navigator.plugins && navigator.plugins.length === 0
      && typeof window.Plugin === 'function' && typeof window.PluginArray === 'function'
      && typeof window.MimeType === 'function' && typeof window.MimeTypeArray === 'function') {
    const mimes = _mimeSpecs.map((m) => _asInterface(Object.assign(Object.create(null), m), window.MimeType)
      || Object.assign({}, m));
    const plugins = _pluginSpecs.map((p) => {
      const plugin = Object.assign(Object.create(null), p, {
        length: mimes.length,
        item: _nativeLike('item', (i) => mimes[i] || null, 1),
        namedItem: _nativeLike('namedItem', (t) => mimes.find((m) => m.type === t) || null, 1),
      });
      mimes.forEach((m, i) => { plugin[i] = m; });
      return _asInterface(plugin, window.Plugin) || plugin;
    });
    mimes.forEach((m) => { try { m.enabledPlugin = plugins[0]; } catch {} });

    _defineGetter(Navigator.prototype, 'plugins', _buildArrayLike(plugins, window.PluginArray, 'name'));
    _defineGetter(Navigator.prototype, 'mimeTypes', _buildArrayLike(mimes, window.MimeTypeArray, 'type'));
  }

  // ── outerWidth / outerHeight ──
  // A headless window can report 0, which no real window does.
  if (!window.outerWidth || !window.outerHeight) {
    _defineGetter(window, 'outerWidth', window.innerWidth);
    _defineGetter(window, 'outerHeight', window.innerHeight + 88);
  }

  // navigator.permissions is deliberately NOT patched: modern headless answers
  // correctly, and the old replacement returned 'default' where Chrome says
  // 'prompt'. Error.prepareStackTrace is deliberately NOT patched either: it is
  // undefined on a real page, so defining it is the stronger signal.
`;
}

/** Stealth alone, for callers that do not need the fingerprint layer. */
function buildStealthScript() {
  return `(function() {\n'use strict';\n${buildMaskPreamble()}\n${buildStealthBody()}\n})();`;
}

module.exports = { buildStealthScript, buildMaskPreamble, buildStealthBody };
