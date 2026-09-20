/**
 * Unit tests for the PageDriver through createPageActions: dispatch, the
 * bounded waits for a tab and a load, and the dev panel's guards.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { createPageActions } = require('../../../../main/page-actions.cjs');
const { PageDriver } = require('../../../../main/actions/driver.cjs');
const {
  TAB_READY_TIMEOUT_MS,
  LOAD_TIMEOUT_MS,
  EMPTY_ANALYSIS_RETRY_MS,
} = require('../../../../main/actions/constants.cjs');
const { pageView, pageCtx, results } = require('../../support/page.cjs');
const { flush } = require('../../support/fakes.cjs');

describe('PageDriver', () => {
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('an action without a handler runs as an analyzer script and answers its result', async () => {
    const view = pageView();
    const ctx = pageCtx(view, { world: { ok: false, data: 1, error: 'e' } });
    await createPageActions(ctx).runPageAction('c1', 'read_page', {}, view);
    assert.deepEqual(ctx.calls[0], ['inject']);
    assert.deepEqual(results(ctx), [['c1', false, 1, 'e']]);
  });

  it('reads a page again when an analysis found no element at all', async () => {
    const view = pageView();
    const pages = [
      { ok: true, data: { elements: [], blocks: [] } },
      { ok: true, data: { elements: [{ id: 1 }], blocks: [] } },
    ];
    const ctx = pageCtx(view, { world: () => pages.shift() });
    mock.timers.enable({ apis: ['setTimeout'] });
    const done = createPageActions(ctx).runPageAction('c1', 'analyze', {}, view);
    await flush();
    mock.timers.tick(EMPTY_ANALYSIS_RETRY_MS);
    await done;
    assert.equal(ctx.calls.filter((c) => c[0] === 'world').length, 2);
    assert.equal(results(ctx)[0][2].elements.length, 1);
  });

  it('reads a page once when the analysis found elements', async () => {
    const view = pageView();
    const ctx = pageCtx(view, { world: { ok: true, data: { elements: [{ id: 1 }], blocks: [] } } });
    await createPageActions(ctx).runPageAction('c1', 'analyze', {}, view);
    assert.equal(ctx.calls.filter((c) => c[0] === 'world').length, 1);
  });

  it('an empty script result counts as success', async () => {
    const view = pageView();
    const ctx = pageCtx(view, { world: null });
    await createPageActions(ctx).runPageAction('c1', 'bogus', {}, view);
    assert.deepEqual(results(ctx), [['c1', true, undefined, undefined]]);
  });

  it('navigate without a url falls through to the script path', async () => {
    const view = pageView();
    const ctx = pageCtx(view, { world: { ok: false, error: 'x' } });
    await createPageActions(ctx).runPageAction('c1', 'navigate', {}, view);
    assert.ok(ctx.calls.some((c) => c[0] === 'world' && c[1].includes('Unknown action: navigate')));
  });

  it('waitForTabReady resolves for a tab with no first load', async () => {
    await createPageActions(pageCtx(pageView())).waitForTabReady({});
  });

  it('waitForTabReady ignores a failed first load', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    await createPageActions(pageCtx(pageView())).waitForTabReady({ ready: Promise.reject(new Error('x')) });
  });

  it('waitForTabReady gives up on a load that never settles', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    let done = false;
    createPageActions(pageCtx(pageView()))
      .waitForTabReady({ ready: new Promise(() => {}) })
      .then(() => (done = true));
    mock.timers.tick(TAB_READY_TIMEOUT_MS - 1);
    await flush();
    assert.equal(done, false);
    mock.timers.tick(1);
    await flush();
    assert.equal(done, true);
  });

  it('waitForLoad settles on a finished load and removes both listeners', async () => {
    const view = pageView();
    const waiting = new PageDriver(pageCtx(view)).waitForLoad(view);
    view.webContents.emit('did-finish-load');
    await waiting;
    assert.equal(view.webContents.listenerCount('did-finish-load'), 0);
    assert.equal(view.webContents.listenerCount('did-fail-load'), 0);
  });

  it('waitForLoad settles on a failed load', async () => {
    const view = pageView();
    const waiting = new PageDriver(pageCtx(view)).waitForLoad(view);
    view.webContents.emit('did-fail-load');
    await waiting;
    assert.equal(view.webContents.listenerCount('did-finish-load'), 0);
  });

  it('waitForLoad gives up after its timeout and cleans up', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const view = pageView();
    const waiting = new PageDriver(pageCtx(view)).waitForLoad(view);
    mock.timers.tick(LOAD_TIMEOUT_MS);
    await waiting;
    assert.equal(view.webContents.listenerCount('did-fail-load'), 0);
  });

  it('waitForLoad returns at once for a destroyed view', async () => {
    const view = pageView();
    view.webContents.destroyed = true;
    await new PageDriver(pageCtx(view)).waitForLoad(view);
  });

  it('a dev action with no active tab is refused, except listing tabs', async () => {
    const ctx = pageCtx(null);
    const actions = createPageActions(ctx);
    assert.deepEqual(await actions.runDevAction('reload'), { ok: false, error: 'No active tab' });
    assert.equal((await actions.runDevAction('list-tabs')).ok, true);
  });

  it('a dev action that acts on the page needs human control', async () => {
    const view = pageView();
    const actions = createPageActions(pageCtx(view, { human: false }));
    assert.deepEqual(await actions.runDevAction('reload'), {
      ok: false,
      error: 'Take control before interacting with this page',
    });
    assert.deepEqual(view.webContents.calls, []);
  });

  it('reading the page needs no human control', async () => {
    const actions = createPageActions(pageCtx(pageView(), { human: false, world: { ok: true } }));
    assert.deepEqual(await actions.runDevAction('analyze'), { ok: true });
    assert.equal((await actions.runDevAction('screenshot')).ok, true);
  });

  it('an unknown dev action, even a prototype name, is reported', async () => {
    const actions = createPageActions(pageCtx(pageView()));
    assert.deepEqual(await actions.runDevAction('toString'), { ok: false, error: 'Unknown action: toString' });
  });

  it('a dev action that throws answers with its message', async () => {
    const view = pageView({ load: async () => Promise.reject(new Error('net down')) });
    const actions = createPageActions(pageCtx(view));
    assert.deepEqual(await actions.runDevAction('navigate', { url: 'x.test' }), { ok: false, error: 'net down' });
  });
});
