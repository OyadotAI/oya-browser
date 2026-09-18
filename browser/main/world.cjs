/**
 * Isolated world.
 *
 * The analyzer runs in its own JS world, not the page's. The page can then
 * neither see our globals (window.analyzePage was a one-line, 100%-precision
 * identifier for this product) nor reach into them. The name is randomised per
 * process so it is not a constant to match on either.
 *
 * An isolated world shares the DOM but has its own globals, so it also gets the
 * UNPATCHED getBoundingClientRect — which is why the analyzer no longer needs a
 * flag to switch the fingerprint noise off while it measures.
 */
const crypto = require('crypto');
const { WORLD_ATTR_BYTES } = require('./constants.cjs');

/**
 * Create (or recreate) the isolated world for this view's main frame and load
 * the analyzer into it. Page.createIsolatedWorld returns the context id
 * directly, so this needs no Runtime.enable — that domain is a detection
 * vector. Only an active recording enables it to receive captured events.
 */
async function ensureIsolatedWorld(world, view, { force = false } = {}) {
  if (!force && world.contexts.has(view)) return world.contexts.get(view);
  const executionContextId = await createWorldContext(world, view);
  world.contexts.set(view, executionContextId);
  await loadAnalyzer(world, view, executionContextId);
  return executionContextId;
}

/** A new isolated world on the view's main frame; its execution context id. */
async function createWorldContext(world, view) {
  const { frameTree } = await world.cdp(view, 'Page.getFrameTree');
  const frameId = frameTree.frame.id;
  const { executionContextId } = await world.cdp(view, 'Page.createIsolatedWorld', {
    frameId,
    worldName: world.worldName,
    grantUniveralAccess: true,
  });
  return executionContextId;
}

/** Runs the analyzer in a fresh world, under a tag attribute of its own. */
async function loadAnalyzer(world, view, contextId) {
  // A fresh tag attribute per document, so the marks the analyzer leaves on the
  // DOM are not a constant any MutationObserver can match on.
  const attr = 'data-' + crypto.randomBytes(WORLD_ATTR_BYTES).toString('hex');
  await world.cdp(view, 'Runtime.evaluate', {
    // RecordingChannel arms new documents while a recording is active.
    expression: world.analyzerScript.replace('__OYA_ATTR__', attr).replace('__OYA_RECORD__', 'false'),
    contextId,
    returnByValue: true,
  });
}

/**
 * Evaluate in the isolated world. Retries once against a fresh world, because
 * a navigation between calls invalidates the context id.
 */
async function evalInWorld(world, view, expression, { retry = true } = {}) {
  const contextId = await ensureIsolatedWorld(world, view);
  let res;
  try {
    res = await world.cdp(view, 'Runtime.evaluate', { expression, contextId, returnByValue: true, awaitPromise: true });
  } catch (err) {
    return evalInFreshWorld(world, view, expression, err, retry);
  }
  return worldValue(res);
}

/** A lost context (a navigation between calls) gets one retry in a fresh world; anything else rethrows. */
async function evalInFreshWorld(world, view, expression, err, retry) {
  if (!retry || !/context|Cannot find/i.test(err.message || '')) throw err;
  await ensureIsolatedWorld(world, view, { force: true });
  return evalInWorld(world, view, expression, { retry: false });
}

/** The evaluation's value, or its exception as an Error. */
function worldValue(res) {
  if (res.exceptionDetails) {
    throw new Error(res.exceptionDetails.exception?.description || res.exceptionDetails.text || 'Evaluation failed');
  }
  return res.result?.value;
}

/** `cdp(view, method, params)` drives the view; `analyzerScript` is loaded into a world called `worldName`. */
function createWorld({ cdp, analyzerScript, worldName }) {
  const world = { cdp, analyzerScript, worldName, contexts: new WeakMap() }; // contexts: view -> executionContextId
  return {
    ensureWorld: (view, options) => ensureIsolatedWorld(world, view, options),
    worldEval: (view, expression, options) => evalInWorld(world, view, expression, options),
  };
}

module.exports = { createWorld };
