/**
 * Unit tests for scripts/target-picker.cjs: picking an element with the
 * inspect overlay, and every way a pick can end.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { pickTarget } = require('../../../scripts/target-picker.cjs');
const { FakeView, flush } = require('../support/fakes.cjs');

/** A view whose page describes the picked node as `element` (or throws `exception`). */
function viewWith(element, { exception, inspectError } = {}) {
  return new FakeView({
    debuggerResponses: {
      'DOM.resolveNode': { object: { objectId: 'o1' } },
      'Runtime.callFunctionOn': exception ? { result: {}, exceptionDetails: {} } : { result: { value: element } },
      'Overlay.setInspectMode': (p) => (p.mode !== 'none' && inspectError ? inspectError : {}),
    },
  });
}

/** Starts a pick and waits until the overlay is on; the pending pick is wrapped so awaiting this does not wait for it. */
async function begin(view) {
  const pick = pickTarget(view);
  pick.catch(() => {});
  await flush();
  return { pick };
}

describe('pickTarget', () => {
  afterEach(() => mock.timers.reset());

  it('attaches, turns on the overlay, and resolves to the picked element’s locators', async () => {
    const view = viewWith({ type: 'button', role: 'button', text: 'Save' });
    const { pick } = await begin(view);
    const dbg = view.webContents.debugger;
    assert.ok(dbg.attached);
    const inspect = dbg.sent.find((c) => c.method === 'Overlay.setInspectMode').params;
    assert.equal(inspect.mode, 'searchForNode');
    assert.deepEqual(inspect.highlightConfig.borderColor, { r: 70, g: 180, b: 160, a: 1 });
    dbg.event('Overlay.inspectNodeRequested', { backendNodeId: 5 });
    assert.deepEqual((await pick)[0], { kind: 'role', role: 'button', value: 'Save' });
    assert.ok(dbg.methods().includes('Runtime.releaseObject'));
    assert.deepEqual(dbg.sent.at(-1).params, { mode: 'none' });
    assert.equal(dbg.listenerCount('message'), 0);
  });

  it('describes the node with a function that climbs to the interactive element', async () => {
    const view = viewWith({ type: 'button', text: 'x' });
    const { pick } = await begin(view);
    view.webContents.debugger.event('Overlay.inspectNodeRequested', { backendNodeId: 5 });
    await pick;
    const call = view.webContents.debugger.sent.find((c) => c.method === 'Runtime.callFunctionOn').params;
    assert.match(
      call.functionDeclaration,
      /this\.closest\('button,a,input,textarea,select,\[role\],\[contenteditable\]'\)/,
    );
    assert.equal(call.returnByValue, true);
  });

  it('ignores other debugger events', async () => {
    const view = viewWith({ type: 'button', text: 'x' });
    const { pick } = await begin(view);
    view.webContents.debugger.event('Page.loadEventFired');
    view.webContents.debugger.event('Overlay.inspectNodeRequested', { backendNodeId: 5 });
    assert.equal((await pick).length, 1);
  });

  it('rejects when the person cancels', async () => {
    const view = viewWith({});
    const { pick } = await begin(view);
    view.webContents.debugger.event('Overlay.inspectModeCanceled');
    await assert.rejects(pick, /canceled/);
  });

  it('rejects an element inside a frame', async () => {
    const view = viewWith({ unsupported: true });
    const { pick } = await begin(view);
    view.webContents.debugger.event('Overlay.inspectNodeRequested', { backendNodeId: 5 });
    await assert.rejects(pick, /top-level targets/);
  });

  it('rejects when the page throws while describing', async () => {
    const view = viewWith(null, { exception: true });
    const { pick } = await begin(view);
    view.webContents.debugger.event('Overlay.inspectNodeRequested', { backendNodeId: 5 });
    await assert.rejects(pick, /top-level targets/);
  });

  it('rejects an element with no stable locator', async () => {
    const view = viewWith({ type: 'div' });
    const { pick } = await begin(view);
    view.webContents.debugger.event('Overlay.inspectNodeRequested', { backendNodeId: 5 });
    await assert.rejects(pick, /no stable target/);
  });

  it('rejects when the page closes', async () => {
    const view = viewWith({});
    const { pick } = await begin(view);
    view.webContents.emit('destroyed');
    await assert.rejects(pick, /page closed/);
  });

  it('rejects when the overlay cannot start', async () => {
    const view = viewWith({}, { inspectError: new Error('no overlay') });
    await assert.rejects(pickTarget(view), /no overlay/);
  });

  it('times out after a minute, settling only once', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const view = viewWith({ type: 'button', text: 'x' });
    const { pick } = await begin(view);
    mock.timers.tick(60000);
    view.webContents.debugger.event('Overlay.inspectModeCanceled');
    await assert.rejects(pick, /timed out/);
    assert.equal(view.webContents.debugger.sent.filter((c) => c.params?.mode === 'none').length, 1);
  });
});
