/**
 * Loads the desktop shell's renderer the way Electron does: the body of
 * index.html in a fake DOM, then every <script src> in order, in one shared
 * global scope (a vm context). `window.oyaBrowser` is a fake that records
 * each call and lets a test fire the main process's events.
 */
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { documentFrom, Event } = require('./fake-dom.cjs');

/** The renderer folder under test. */
const RENDERER = path.join(__dirname, '..', '..', '..', 'renderer');

/** Resolves once pending promise callbacks and immediates have run. */
const settle = async (rounds = 3) => {
  for (let i = 0; i < rounds; i++) await new Promise((resolve) => setImmediate(resolve));
};

/**
 * The preload bridge. Calls are recorded in `calls` and answered from
 * `answers` (a value or a function of the arguments); `onX(fn)` subscribes and
 * `emit('X', …)` delivers to every subscriber.
 */
function fakeBridge(answers = {}) {
  const calls = [];
  const listeners = {};
  const api = {
    calls,
    answers,
    emit: (name, ...args) => (listeners[name] || []).forEach((fn) => fn(...args)),
    called: (name) => calls.filter((c) => c.name === name).map((c) => c.args),
  };
  return new Proxy(api, {
    get(target, name) {
      if (name in target || typeof name !== 'string') return target[name];
      if (/^on[A-Z]/.test(name)) return (fn) => (listeners[name.slice(2)] ||= []).push(fn);
      return (...args) => {
        calls.push({ name, args });
        const answer = answers[name];
        return Promise.resolve(typeof answer === 'function' ? answer(...args) : answer);
      };
    },
  });
}

/** Default answers the renderer expects at load. */
const DEFAULT_ANSWERS = {
  getConfig: { serverUrl: '', apiKey: '', browserName: '' },
  getStatus: { connected: false, url: '' },
  getUpdateStatus: { state: 'idle', current: '1.0.0' },
  getUiPreferences: { theme: 'system', platform: 'darwin' },
  getControlState: { mode: 'offline', interactive: true },
  workspace: () => undefined,
};

/** The window's globals: timers, animation frames, media queries, navigator. */
function windowGlobals(document, bridge, options) {
  const frames = [];
  const clipboard = { written: [], writeText: async (text) => clipboard.written.push(text) };
  const listeners = {};
  return {
    addEventListener: (type, fn) => (listeners[type] ||= []).push(fn),
    dispatchWindow: (type) => (listeners[type] || []).forEach((fn) => fn(new Event(type))),
    document,
    oyaBrowser: bridge,
    navigator: { platform: options.platform || 'MacIntel', clipboard },
    innerWidth: options.innerWidth ?? 1280,
    innerHeight: options.innerHeight ?? 800,
    matchMedia: () => ({ matches: !!options.dark }),
    getSelection: () => ({ toString: () => options.selection?.() || '' }),
    requestAnimationFrame: (fn) => frames.push(fn),
    cancelAnimationFrame: () => {},
    runFrames: () => frames.splice(0).forEach((fn) => fn()),
    ...{ setTimeout, clearTimeout, setInterval, clearInterval, console, Event },
  };
}

/** The script files index.html loads, in order. */
function scriptsOf(html) {
  return [...html.matchAll(/<script src="([^"]+)"/g)].map((m) => m[1]);
}

/**
 * Loads `page` (default index.html) of the renderer in `root` (default: the
 * real one). Returns the window, document and bridge, plus helpers to query
 * and fire events.
 */
function loadRenderer({ root = RENDERER, page = 'index.html', answers = {}, ...options } = {}) {
  const html = fs.readFileSync(path.join(root, page), 'utf8');
  const document = documentFrom(html);
  const bridge = fakeBridge({ ...DEFAULT_ANSWERS, ...answers });
  const window = vm.createContext(windowGlobals(document, bridge, options));
  window.window = window;
  document.defaultView = window;
  for (const file of scriptsOf(html)) {
    vm.runInContext(fs.readFileSync(path.join(root, file), 'utf8'), window, { filename: file });
  }
  const $ = (id) => document.getElementById(id);
  const fire = (el, type, init) => el.dispatchEvent(new Event(type, init));
  const key = (el, k, init = {}) => fire(el, 'keydown', { key: k, ...init });
  return { window, document, bridge, $, fire, key, settle, run: (code) => vm.runInContext(code, window) };
}

module.exports = { loadRenderer, fakeBridge, settle, RENDERER };
