/**
 * Unit tests for main/world.cjs: the analyzer lives in an isolated world per
 * view, reused until forced, and an evaluation survives one lost context.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { createWorld } = require('../../../main/world.cjs');

/** A cdp() double: answers by method, records each call. */
function fakeCdp(answers) {
  const calls = [];
  let contextId = 0;
  const cdp = async (view, method, params) => {
    calls.push({ method, params });
    if (method === 'Page.getFrameTree') return { frameTree: { frame: { id: 'main' } } };
    if (method === 'Page.createIsolatedWorld') return { executionContextId: ++contextId };
    const answer = answers.shift();
    if (answer instanceof Error) throw answer;
    return answer ?? {};
  };
  return { cdp, calls };
}

/** The analyzer loads are the evaluations without awaitPromise. */
const analyzerLoads = (calls) => calls.filter((c) => c.method === 'Runtime.evaluate' && !c.params.awaitPromise);

describe('createWorld', () => {
  const analyzerScript = 'attr=__OYA_ATTR__;record=__OYA_RECORD__';

  it('creates the world once per view and loads the analyzer with a fresh attribute and recording off', async () => {
    const { cdp, calls } = fakeCdp([]);
    const { ensureWorld } = createWorld({ cdp, analyzerScript, worldName: 'w1' });
    const view = {};
    assert.equal(await ensureWorld(view), 1);
    assert.equal(await ensureWorld(view), 1);
    assert.deepEqual(calls[1].params, { frameId: 'main', worldName: 'w1', grantUniveralAccess: true });
    const [load] = analyzerLoads(calls);
    assert.match(load.params.expression, /^attr=data-[0-9a-f]{8};record=false$/);
    assert.equal(load.params.contextId, 1);
  });

  it('rebuilds the world when forced', async () => {
    const { cdp } = fakeCdp([]);
    const { ensureWorld } = createWorld({ cdp, analyzerScript, worldName: 'w' });
    const view = {};
    await ensureWorld(view);
    assert.equal(await ensureWorld(view, { force: true }), 2);
  });

  it('evaluates in the world and returns the value', async () => {
    const { cdp, calls } = fakeCdp([{}, { result: { value: 'ok' } }]);
    const { worldEval } = createWorld({ cdp, analyzerScript, worldName: 'w' });
    assert.equal(await worldEval({}, '1'), 'ok');
    assert.deepEqual(calls.at(-1).params, { expression: '1', contextId: 1, returnByValue: true, awaitPromise: true });
  });

  it('retries once in a fresh world when the context was lost to a navigation', async () => {
    const lost = new Error('Cannot find context with specified id');
    const { cdp } = fakeCdp([{}, lost, {}, { result: { value: 'again' } }]);
    const { worldEval } = createWorld({ cdp, analyzerScript, worldName: 'w' });
    assert.equal(await worldEval({}, '1'), 'again');
  });

  it('gives up after the one retry', async () => {
    const lost = new Error('Cannot find context');
    const { cdp } = fakeCdp([{}, lost, {}, lost]);
    const { worldEval } = createWorld({ cdp, analyzerScript, worldName: 'w' });
    await assert.rejects(worldEval({}, '1'), /Cannot find context/);
  });

  it('rethrows any other failure without retrying', async () => {
    const { cdp, calls } = fakeCdp([{}, new Error('Target closed')]);
    const { worldEval } = createWorld({ cdp, analyzerScript, worldName: 'w' });
    await assert.rejects(worldEval({}, '1'), /Target closed/);
    assert.equal(calls.filter((c) => c.method === 'Page.createIsolatedWorld').length, 1);
  });

  it('throws the page exception of a failed evaluation', async () => {
    const thrown = { exceptionDetails: { exception: { description: 'TypeError: x' }, text: 'Uncaught' } };
    const { cdp } = fakeCdp([{}, thrown]);
    const { worldEval } = createWorld({ cdp, analyzerScript, worldName: 'w' });
    await assert.rejects(worldEval({}, 'x'), /TypeError: x/);
  });
});
