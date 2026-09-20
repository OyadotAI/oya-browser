/**
 * Helpers for the page-command tests: a view whose debugger answers, a ctx
 * like main.js's, timers that fire at once and record their delays, and a
 * Math.random that repeats one value.
 */
const { mock } = require('node:test');
const { FakeView } = require('./fakes.cjs');

/**
 * Makes every setTimeout fire on the next turn and records its delay, so a
 * chain of human-like pauses runs instantly. Returns the recorded delays.
 */
function instantTimers() {
  const delays = [];
  mock.method(globalThis, 'setTimeout', (fn, ms) => {
    delays.push(ms);
    setImmediate(fn);
    return 0;
  });
  mock.method(globalThis, 'clearTimeout', () => {});
  return delays;
}

/** Pins Math.random to `value` for the rest of the test. */
const fixedRandom = (value) => mock.method(Math, 'random', () => value);

/**
 * A view whose debugger answers Runtime.evaluate with `evalValue` and a
 * screenshot with `PNG`, and whose webContents loads, reloads and takes input.
 */
function pageView({ evalValue = { w: 1000, h: 700 }, loading = false, load } = {}) {
  const view = new FakeView({
    url: 'https://a.test/',
    debuggerResponses: {
      'Runtime.evaluate': { result: { value: evalValue } },
      'Page.captureScreenshot': { data: 'PNG' },
    },
  });
  const wc = view.webContents;
  Object.assign(wc, { calls: [], getTitle: () => 'Title', isLoading: () => loading });
  wc.loadURL = load || (async (url) => wc.calls.push(['loadURL', url]));
  wc.sendInputEvent = (event) => wc.calls.push(['input', event]);
  wc.insertText = async (text) => wc.calls.push(['insert', text]);
  wc.reload = () => wc.calls.push(['reload']);
  return view;
}

/**
 * main.js's side of createPageActions: `world` answers worldEval (a value, or
 * a function of the expression), and every call is recorded in `calls`.
 */
function pageCtx(view, { world, human = true, tabs } = {}) {
  const calls = [];
  const record =
    (name) =>
    (...args) =>
      calls.push([name, ...args]);
  const ctx = {
    calls,
    getActiveView: () => view,
    pullCookiesFor: async (url) => calls.push(['pull', url]),
    injectScripts: async () => calls.push(['inject']),
    worldEval: async (v, expr) => (calls.push(['world', expr]), typeof world === 'function' ? world(expr) : world),
    sendResult: record('result'),
    createTab: (url, activate) => (calls.push(['createTab', url, activate]), 7),
    closeTab: record('closeTab'),
    requireHumanControl: () => {
      if (!human) throw new Error('Take control before interacting with this page');
    },
    tabs: () => tabs || [{ id: 1, view, title: 'Title', url: 'https://a.test/' }],
    activeTabId: () => 1,
  };
  return ctx;
}

/** The sendResult calls, as [id, ok, data, error]. */
const results = (ctx) => ctx.calls.filter((c) => c[0] === 'result').map((c) => c.slice(1));

/** The Input.dispatchMouseEvent params sent, in order. */
const mouseEvents = (view) =>
  view.webContents.debugger.sent.filter((c) => c.method === 'Input.dispatchMouseEvent').map((c) => c.params);

module.exports = { instantTimers, fixedRandom, pageView, pageCtx, results, mouseEvents };
