/**
 * Where page scripts run: the analyzer's isolated world, or the page's own
 * main world when a script has to reach page globals.
 */
import { analyzerSource, getAnalyzer } from './browser-scripts.ts';
import type { CDPDriver } from './driver.ts';

/** Errors that mean a navigation destroyed the world, not that the script failed. */
const WORLD_LOST = /context|Cannot find/i;

/**
 * A world that shares the DOM but not the page's globals. Page.createIsolatedWorld
 * hands back the context id directly, so this needs no Runtime.enable — which
 * is itself a detection vector.
 */
export async function ensureWorld(driver: CDPDriver, { force = false } = {}) {
  if (!force && driver.worldContext) return driver.worldContext;
  const analyzer = getAnalyzer();
  if (!analyzer) throw new Error('Analyzer unavailable for this client');
  const executionContextId = await createWorld(driver);
  driver.worldContext = executionContextId;
  await loadAnalyzer(driver, analyzer, executionContextId);
  return executionContextId;
}

/** Creates the isolated world in the page's main frame and returns its context id. */
async function createWorld(driver: CDPDriver) {
  const { frameTree } = await driver.conn.send('Page.getFrameTree', {}, driver.sessionId);
  const world = { frameId: frameTree.frame.id, worldName: driver.worldName, grantUniveralAccess: true };
  const { executionContextId } = await driver.conn.send('Page.createIsolatedWorld', world, driver.sessionId);
  return executionContextId;
}

/** Runs the analyzer in the new world. RecordingChannel arms new documents while a recording is active. */
async function loadAnalyzer(driver: CDPDriver, analyzer, contextId) {
  const expression = analyzerSource(analyzer, driver.tagAttr);
  await driver.conn.send('Runtime.evaluate', { expression, contextId, returnByValue: true }, driver.sessionId);
}

/** Everything the analyzer needs runs here, never in the page's own world. */
export async function evaluateInWorld(driver: CDPDriver, expression, { awaitPromise = true, retry = true } = {}) {
  const contextId = await driver.ensureWorld();
  let res;
  try {
    res = await runtimeEvaluate(driver, expression, awaitPromise, contextId);
  } catch (err) {
    return retryInNewWorld(driver, err, expression, { awaitPromise, retry });
  }
  return resultValue(res);
}

/**
 * A navigation destroys the world; rebuild it once rather than failing the
 * command the user actually asked for. Any other error is the caller's.
 */
async function retryInNewWorld(driver: CDPDriver, err, expression, { awaitPromise, retry }) {
  if (!retry || !WORLD_LOST.test(err.message || '')) throw err;
  await driver.ensureWorld({ force: true });
  return driver.evaluate(expression, { awaitPromise, retry: false });
}

/**
 * The page's own world. CAPTCHA and MFA handling has to reach page globals —
 * `___grecaptcha_cfg.clients[…].callback` is a function the page defined, and
 * an isolated world cannot see it — so those scripts run here. Nothing from
 * this driver is left behind in it.
 */
export async function evaluateInMain(driver: CDPDriver, expression, { awaitPromise = true } = {}) {
  const res = await runtimeEvaluate(driver, expression, awaitPromise);
  return resultValue(res);
}

/** One Runtime.evaluate in the attached target: in a given world, or the main one when contextId is absent. */
function runtimeEvaluate(driver: CDPDriver, expression, awaitPromise, contextId?) {
  const params = { expression, contextId, returnByValue: true, awaitPromise, userGesture: true };
  return driver.conn.send('Runtime.evaluate', params, driver.sessionId);
}

/** The value an evaluation returned; a thrown page exception becomes an Error. */
function resultValue(res) {
  if (res.exceptionDetails) throw new Error(res.exceptionDetails.exception?.description || 'Evaluation failed');
  return res.result?.value;
}
