/**
 * PageDriver: what every page command shares. It holds main.js's tab state
 * and helpers, waits for loads, finds elements, and dispatches server and dev
 * panel commands through their command maps.
 */
const { sleep } = require('../input.cjs');
const { findElementJs, actionScript } = require('./scripts.cjs');
const { renderedAnalysis } = require('../page-format.cjs');
const { TAB_READY_TIMEOUT_MS, LOAD_TIMEOUT_MS } = require('./constants.cjs');
const { PAGE_COMMANDS } = require('./page-commands.cjs');
const { DEV_COMMANDS, UNGUARDED_DEV_COMMANDS } = require('./dev-commands.cjs');

/** The events that end a load, successful or not. */
const LOAD_END_EVENTS = ['did-finish-load', 'did-fail-load'];

/** Calls `resolve` once `contents` ends a load or `ms` pass, removing its listeners either way. */
function untilLoaded(contents, ms, resolve) {
  const done = () => {
    clearTimeout(timer);
    if (!contents.isDestroyed()) for (const event of LOAD_END_EVENTS) contents.off(event, done);
    resolve();
  };
  const timer = setTimeout(done, ms);
  for (const event of LOAD_END_EVENTS) contents.once(event, done);
}

/** Drives the active page for the command socket and the dev panel. */
class PageDriver {
  /** `ctx` is main.js's tab state (read through getters) and the helpers that act on it. */
  constructor(ctx) {
    /** The tab state and helpers from main.js. */
    this.ctx = ctx;
  }

  /**
   * Wait for a tab's first load before driving it — but never unconditionally.
   *
   * tab.ready only settles once CDP setup and the initial navigation finish. A
   * page that never finishes doing either used to block every later command on
   * that tab with no result and no error, so the caller just timed out. That is
   * how a browser image whose renderers would not start looked like a dead
   * server rather than a broken page.
   */
  waitForTabReady(tab) {
    if (!tab?.ready) return Promise.resolve();
    // A rejected first load is not this command's problem: it is about to
    // navigate somewhere else anyway.
    return Promise.race([tab.ready.catch(() => {}), sleep(TAB_READY_TIMEOUT_MS)]);
  }

  /**
   * Settle when the page finishes or fails loading, or give up.
   *
   * Every listener is removed on the way out. The old version attached one per
   * call and dropped it only when the load fired, so an agent session driving a
   * page that never finishes piled them onto the same webContents.
   */
  waitForLoad(view = this.ctx.getActiveView(), ms = LOAD_TIMEOUT_MS) {
    if (!view || view.webContents.isDestroyed()) return Promise.resolve();
    return new Promise((resolve) => untilLoaded(view.webContents, ms, resolve));
  }

  /** Loads the analyzer, then finds `selector`: `{ ok, data: { x, y, tag, editable, inIframe } }` or `{ ok: false, error }`. */
  async locate(view, selector) {
    await this.ctx.injectScripts(view);
    return this.ctx.worldEval(view, findElementJs(selector));
  }

  /** Like locate(), but a missing element is answered to the caller here; resolves to the element or null. */
  async find(id, view, selector) {
    const info = await this.locate(view, selector);
    if (info?.ok) return info;
    this.ctx.sendResult(id, false, null, info?.error || 'Element not found');
    return null;
  }

  /** Everything after tab management in runCommand; answers through sendResult. */
  async runPageAction(id, action, params, view) {
    if (!Object.hasOwn(PAGE_COMMANDS, action)) return this.runInjected(id, action, params, view);
    await PAGE_COMMANDS[action](this, id, params, view);
  }

  /** An action with no handler of its own runs as an analyzer script. */
  async runInjected(id, action, params, view) {
    await this.ctx.injectScripts(view);
    const raw = await this.ctx.worldEval(view, actionScript(action, params));
    const result = action === 'analyze' ? renderedAnalysis(this.ctx, raw, params) : raw;
    this.ctx.sendResult(id, result?.ok ?? true, result?.data, result?.error);
  }

  /** The dev panel's quick actions; answers by return value, never throws. */
  async runDevAction(action, params) {
    const view = this.ctx.getActiveView();
    if (!view && action !== 'list-tabs') return { ok: false, error: 'No active tab' };
    try {
      return await this.devCommand(action, view, params);
    } catch (err) {
      return { ok: false, error: err.message };
    }
  }

  /** Runs one dev action; everything but a read needs human control. */
  devCommand(action, view, params) {
    if (!UNGUARDED_DEV_COMMANDS.includes(action)) this.ctx.requireHumanControl();
    if (!Object.hasOwn(DEV_COMMANDS, action)) return { ok: false, error: 'Unknown action: ' + action };
    return DEV_COMMANDS[action](this, view, params);
  }
}

module.exports = { PageDriver };
