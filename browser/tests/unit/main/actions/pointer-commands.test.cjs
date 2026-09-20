/**
 * Unit tests for the pointer and raw keyboard commands: coordinate clicks,
 * moves, double clicks, drags, scrolls and untargeted typing.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { createPageActions } = require('../../../../main/page-actions.cjs');
const c = require('../../../../main/actions/constants.cjs');
const { instantTimers, fixedRandom, pageView, pageCtx, results, mouseEvents } = require('../../support/page.cjs');

/** Runs one server command against `view` and returns the ctx that recorded it. */
async function run(view, action, params, options) {
  const ctx = pageCtx(view, options);
  await createPageActions(ctx).runPageAction('c1', action, params, view);
  return ctx;
}

describe('pointer commands', () => {
  let delays;
  beforeEach(() => {
    delays = instantTimers();
    fixedRandom(0);
  });
  afterEach(() => mock.restoreAll());

  it('click_coordinates defaults to the origin and reports the page', async () => {
    const ctx = await run(pageView(), 'click_coordinates', {});
    assert.deepEqual(results(ctx), [
      ['c1', true, { clicked: true, x: 0, y: 0, url: 'https://a.test/', title: 'Title' }],
    ]);
  });

  it('mouse_move ends on the point', async () => {
    const view = pageView();
    const ctx = await run(view, 'mouse_move', { x: 30, y: 40 });
    assert.deepEqual(mouseEvents(view).at(-1), { type: 'mouseMoved', x: 30, y: 40 });
    assert.deepEqual(results(ctx), [['c1', true, { moved: true, x: 30, y: 40 }]]);
  });

  it('double_click needs coordinates or a selector', async () => {
    const ctx = await run(pageView(), 'double_click', {});
    assert.deepEqual(results(ctx), [['c1', false, null, 'Provide x,y coordinates or element_id']]);
  });

  it('double_click on a missing element answers its error', async () => {
    const ctx = await run(pageView(), 'double_click', { selector: 'q' }, { world: { ok: false, error: 'gone' } });
    assert.deepEqual(results(ctx), [['c1', false, null, 'gone']]);
  });

  it('double_click clicks once, then again as click two', async () => {
    const view = pageView();
    const ctx = await run(view, 'double_click', { selector: 'q' }, { world: { ok: true, data: { x: 3, y: 4 } } });
    const presses = mouseEvents(view).filter((e) => e.type !== 'mouseMoved');
    assert.deepEqual(
      presses.map((e) => [e.type, e.clickCount]),
      [
        ['mousePressed', 1],
        ['mouseReleased', 1],
        ['mousePressed', c.DOUBLE_CLICK],
        ['mouseReleased', c.DOUBLE_CLICK],
      ],
    );
    assert.ok(delays.includes(c.DOUBLE_CLICK_GAP.base));
    assert.deepEqual(results(ctx), [['c1', true, { double_clicked: true, x: 3, y: 4 }]]);
  });

  it('keyboard_type with no text answers at once', async () => {
    const view = pageView();
    const ctx = await run(view, 'keyboard_type', {});
    assert.deepEqual(view.webContents.debugger.sent, []);
    assert.deepEqual(results(ctx), [['c1', true, { typed: true }]]);
  });

  it('keyboard_type types the text', async () => {
    const ctx = await run(pageView(), 'keyboard_type', { text: 'ok' });
    assert.deepEqual(results(ctx), [['c1', true, { typed: true, text: 'ok' }]]);
  });

  it('drag holds the button along a straight line and releases at the end', async () => {
    const view = pageView();
    const ctx = await run(view, 'drag', { from_x: 0, from_y: 0, to_x: 100, to_y: 50 });
    const events = mouseEvents(view);
    const pressed = events.findIndex((e) => e.type === 'mousePressed');
    const held = events.slice(pressed + 1, -1);
    assert.equal(held.length, c.DRAG_STEPS);
    assert.ok(held.every((e) => e.buttons === 1));
    assert.deepEqual(held.at(-1), { type: 'mouseMoved', x: 100, y: 50, button: 'left', buttons: 1 });
    assert.deepEqual(events.at(-1), { type: 'mouseReleased', x: 100, y: 50, button: 'left' });
    assert.deepEqual(results(ctx)[0][2], { dragged: true, from: { x: 0, y: 0 }, to: { x: 100, y: 50 } });
  });

  it('a live-control scroll is one wheel event at the pointer, with no analysis', async () => {
    const view = pageView();
    const ctx = await run(view, 'scroll', { smooth: false, x: 5, y: 6, amount: 40, direction: 'up' });
    assert.deepEqual(mouseEvents(view), [{ type: 'mouseWheel', x: 5, y: 6, deltaX: 0, deltaY: -40 }]);
    assert.deepEqual(results(ctx), [['c1', true, { direction: 'up', amount: 40 }]]);
  });

  it('a live-control scroll with a bad amount scrolls nothing', async () => {
    const view = pageView();
    await run(view, 'scroll', { smooth: false, x: 5, y: 6, amount: 'lots' });
    assert.equal(mouseEvents(view)[0].deltaY, 0);
  });

  it('a smooth scroll splits the distance across notches from the viewport centre', async () => {
    const view = pageView({ evalValue: { w: 1000, h: 700 } });
    const ctx = await run(view, 'scroll', { amount: 600 }, { world: { ok: true, data: { d: 1 } } });
    const wheels = mouseEvents(view);
    assert.equal(wheels.length, 600 / c.SCROLL_STEP_PX);
    assert.ok(wheels.every((e) => e.x === 500 && e.y === 350 && e.deltaY === c.SCROLL_STEP_PX));
    assert.deepEqual(results(ctx), [['c1', true, { d: 1 }, undefined]]);
  });

  it('a short smooth scroll still takes the minimum steps, with fallback viewport', async () => {
    const view = pageView({ evalValue: null });
    await run(view, 'scroll', { amount: 10, direction: 'up' }, { world: null });
    const wheels = mouseEvents(view);
    assert.equal(wheels.length, c.MIN_SCROLL_STEPS);
    assert.deepEqual([wheels[0].x, wheels[0].y], [c.FALLBACK_VIEWPORT.w / c.HALF, c.FALLBACK_VIEWPORT.h / c.HALF]);
    assert.ok(wheels.every((e) => e.deltaY < 0));
  });
});
