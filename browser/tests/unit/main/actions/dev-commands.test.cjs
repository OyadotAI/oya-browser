/**
 * Unit tests for the dev panel's quick actions: each validates its input,
 * acts on the page, and answers `{ ok, data?, error? }`.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { createPageActions } = require('../../../../main/page-actions.cjs');
const c = require('../../../../main/actions/constants.cjs');
const { instantTimers, fixedRandom, pageView, pageCtx, mouseEvents } = require('../../support/page.cjs');

/** Runs one dev action against a view; returns its answer and the recorders. */
async function dev(action, params, { view = pageView(), ...options } = {}) {
  const ctx = pageCtx(view, options);
  return { answer: await createPageActions(ctx).runDevAction(action, params), ctx, view };
}

describe('dev commands', () => {
  beforeEach(() => {
    instantTimers();
    fixedRandom(0);
  });
  afterEach(() => mock.restoreAll());

  it('required inputs are checked before acting', async () => {
    for (const [action, error] of [
      ['navigate', 'URL required'],
      ['click', 'element_id required'],
      ['type', 'element_id and text required'],
      ['press-key', 'key required'],
      ['hover', 'element_id required'],
      ['click-coords', 'x and y required'],
      ['wait', 'selector required'],
      ['select', 'element_id and value required'],
    ]) {
      assert.deepEqual((await dev(action, {})).answer, { ok: false, error }, action);
    }
  });

  it('navigate adds https:// and reports the page', async () => {
    const { answer, view, ctx } = await dev('navigate', { url: 'x.test' });
    assert.deepEqual(view.webContents.calls, [['loadURL', 'https://x.test']]);
    assert.deepEqual(ctx.calls[0], ['pull', 'https://x.test']);
    assert.deepEqual(answer, { ok: true, data: { url: 'https://a.test/', title: 'Title' } });
  });

  it('click finds the element by analyzer id', async () => {
    const { answer, ctx } = await dev('click', { element_id: 4 }, { world: { ok: true, data: { x: 1, y: 2 } } });
    assert.ok(ctx.calls.some((call) => call[0] === 'world' && call[1].includes('f("[data-ac-id=\\"4\\"]")')));
    assert.deepEqual(answer, { ok: true, data: { clicked: true, url: 'https://a.test/' } });
  });

  it('click, type and hover report a missing element', async () => {
    for (const action of ['click', 'type', 'hover']) {
      const { answer } = await dev(action, { element_id: 4, text: 't' }, { world: null });
      assert.deepEqual(answer, { ok: false, error: 'Element not found' }, action);
    }
  });

  it('type clicks, clears and types', async () => {
    const { answer, view } = await dev(
      'type',
      { element_id: 4, text: 'x' },
      { world: { ok: true, data: { x: 1, y: 2 } } },
    );
    const keys = view.webContents.debugger.sent.filter((call) => call.method === 'Input.dispatchKeyEvent');
    assert.deepEqual(
      keys.filter((k) => k.params.type !== 'keyUp').map((k) => k.params.key),
      ['a', 'Backspace', 'x'],
    );
    assert.deepEqual(answer, { ok: true, data: { typed: true } });
  });

  it('press-key presses the key', async () => {
    assert.deepEqual((await dev('press-key', { key: 'Tab' })).answer, { ok: true, data: { key: 'Tab' } });
  });

  it('hover moves onto the element', async () => {
    const { answer, view } = await dev('hover', { element_id: 1 }, { world: { ok: true, data: { x: 7.4, y: 8.6 } } });
    assert.deepEqual(mouseEvents(view).at(-1), { type: 'mouseMoved', x: 7, y: 9 });
    assert.deepEqual(answer, { ok: true, data: { hovered: true } });
  });

  it('click-coords clicks the point', async () => {
    assert.deepEqual((await dev('click-coords', { x: 0, y: 0 })).answer, {
      ok: true,
      data: { clicked: true, x: 0, y: 0 },
    });
  });

  it('scroll-down and scroll-up wheel from the viewport centre by the default amount', async () => {
    const down = await dev('scroll-down', {});
    const up = await dev('scroll-up', {});
    assert.deepEqual(mouseEvents(down.view), [
      { type: 'mouseWheel', x: 500, y: 350, deltaX: 0, deltaY: c.DEV_SCROLL_AMOUNT },
    ]);
    assert.equal(mouseEvents(up.view)[0].deltaY, -c.DEV_SCROLL_AMOUNT);
    assert.deepEqual(down.answer, { ok: true });
  });

  it('reload reloads the page', async () => {
    const { answer, view } = await dev('reload');
    assert.deepEqual(view.webContents.calls, [['reload']]);
    assert.deepEqual(answer, { ok: true });
  });

  it('evaluate_raw answers the page-world value', async () => {
    const { answer } = await dev('evaluate_raw', { expression: '1' }, { view: pageView({ evalValue: 1 }) });
    assert.deepEqual(answer, { ok: true, data: { result: 1 } });
  });

  it('screenshot answers a PNG data URL', async () => {
    assert.deepEqual((await dev('screenshot')).answer, { ok: true, data: { screenshot: 'data:image/png;base64,PNG' } });
  });

  it('handle_dialog with no dialog open says so', async () => {
    assert.deepEqual((await dev('handle_dialog', {})).answer, { ok: false, error: 'No dialog is open' });
  });

  it('wait and select answer the script result', async () => {
    assert.deepEqual((await dev('wait', { selector: 'a' }, { world: { ok: false, error: 'Timeout' } })).answer, {
      ok: false,
      error: 'Timeout',
    });
    assert.deepEqual((await dev('select', { element_id: 1, value: 'v' }, { world: { ok: true } })).answer, {
      ok: true,
    });
  });

  it('list-tabs marks the active tab', async () => {
    const tabs = [
      { id: 1, title: 'A', url: 'a' },
      { id: 2, title: 'B', url: 'b' },
    ];
    const { answer } = await dev('list-tabs', {}, { tabs });
    assert.deepEqual(answer.data.tabs, [
      { id: 1, title: 'A', url: 'a', active: true },
      { id: 2, title: 'B', url: 'b', active: false },
    ]);
  });

  it('new-tab opens about:blank by default and close-tab closes the active tab', async () => {
    const opened = await dev('new-tab', {});
    assert.deepEqual(opened.ctx.calls, [['createTab', 'about:blank', true]]);
    assert.deepEqual(opened.answer, { ok: true, data: { tab_id: 7 } });
    const closed = await dev('close-tab', {});
    assert.deepEqual(closed.ctx.calls, [['closeTab', 1]]);
  });
});
