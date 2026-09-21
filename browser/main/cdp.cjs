/** Chrome DevTools Protocol on an Electron view's own debugger. */

const { DEBUGGER_MAX_LISTENERS } = require('./constants.cjs');

const CDP_VERSION = '1.3';

/** Attaches a debugger; an attach that fails is left for the command to report. */
function attachQuietly(dbg) {
  try {
    dbg.attach(CDP_VERSION);
  } catch {}
}

/** The view's debugger, attached; null once the view is gone. */
function cdpAttach(view) {
  if (!view || view.webContents.isDestroyed()) return null;
  const dbg = view.webContents.debugger;
  dbg.setMaxListeners?.(DEBUGGER_MAX_LISTENERS);
  if (!dbg.isAttached()) attachQuietly(dbg);
  return dbg;
}

/** Sends one CDP command to the view. */
async function cdp(view, method, params = {}) {
  const dbg = cdpAttach(view);
  if (!dbg) throw new Error('View is destroyed');
  return dbg.sendCommand(method, params);
}

/** Evaluates `expression` in the page's main world and returns its value; a thrown exception rejects. */
async function cdpEval(view, expression) {
  const res = await cdp(view, 'Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (res.exceptionDetails) throw new Error(res.exceptionDetails.text || 'Eval failed');
  return res.result?.value;
}

module.exports = { CDP_VERSION, cdpAttach, cdp, cdpEval };
