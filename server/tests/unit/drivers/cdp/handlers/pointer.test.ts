/**
 * Unit tests for the pointer handlers against a fake CDP connection: each one
 * sends the mouse events its gesture is made of, at the right point.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { DRAG_STEPS, SCROLL_PAGE_FRACTION, WHEEL_ORIGIN } from '../../../../../src/drivers/cdp/constants.ts';
import { Status } from '../../../../../src/platform/http-status.ts';
import { SESSION, fakeDriver } from '../../../support/cdp.ts';

/** A driver whose page finds every element at (40, 60) and reports its URL. */
function pageWithElement() {
  const made = fakeDriver();
  made.conn.evaluate = (expression) => {
    if (expression.includes('scrollIntoView')) return { ok: true, data: { x: 40, y: 60 } };
    if (expression === 'location.href') return 'https://site.example/next';
    if (expression.includes('innerWidth')) return { width: 1000, height: 500 };
    return undefined;
  };
  return made;
}

/** The mouse events sent, as [type, x, y, clickCount]. */
const mouse = (conn: any) =>
  conn.sent('Input.dispatchMouseEvent').map((c: any) => [c.params.type, c.params.x, c.params.y, c.params.clickCount]);

describe('click', () => {
  it('locates the analyzer element, presses and releases at its centre, and reports the URL', async () => {
    const { driver, conn } = pageWithElement();
    const result = await driver.dispatch('click', { element_id: 3 });
    assert.deepEqual(result, { ok: true, data: { clicked: true, url: 'https://site.example/next' } });
    assert.deepEqual(mouse(conn), [
      ['mousePressed', 40, 60, 1],
      ['mouseReleased', 40, 60, 1],
    ]);
    assert.ok(conn.sent('Runtime.evaluate').some((c) => c.params.expression.includes('[data-ac-id=\\"3\\"]')));
    assert.ok(conn.sent('Input.dispatchMouseEvent').every((c) => c.sessionId === SESSION));
  });

  it('clicks a selector, which is what the agent sends', async () => {
    const { driver, conn } = pageWithElement();
    const result = await driver.dispatch('click', { selector: '[data-ac-id="13"]' });
    assert.deepEqual(result, { ok: true, data: { clicked: true, url: 'https://site.example/next' } });
    assert.deepEqual(mouse(conn), [
      ['mousePressed', 40, 60, 1],
      ['mouseReleased', 40, 60, 1],
    ]);
  });

  it('requires an element id', async () => {
    const { driver } = pageWithElement();
    assert.deepEqual(await driver.dispatch('click', {}), { ok: false, error: 'element_id required' });
  });

  it('refuses an element id that is not an analyzer id', async () => {
    const { driver } = pageWithElement();
    await assert.rejects(driver.dispatch('click', { element_id: '1"]; alert(1); ["' }), {
      status: Status.BAD_REQUEST,
    });
  });

  it('fails when the element is not on the page', async () => {
    const { driver, conn } = fakeDriver();
    conn.evaluate = () => ({ ok: false, error: 'Element not found' });
    await assert.rejects(driver.dispatch('click', { element_id: 9 }), { message: 'Element not found' });
  });
});

describe('coordinate gestures', () => {
  it('clicks at coordinates', async () => {
    const { driver, conn } = fakeDriver();
    assert.deepEqual(await driver.dispatch('click-coords', { x: '5', y: 6 }), { ok: true, data: { clicked: true } });
    assert.deepEqual(mouse(conn), [
      ['mousePressed', 5, 6, 1],
      ['mouseReleased', 5, 6, 1],
    ]);
  });

  it('moves the pointer, treating missing coordinates as zero', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('mouse_move', { x: 7 });
    assert.deepEqual(mouse(conn), [['mouseMoved', 7, 0, 1]]);
  });

  it('double-clicks as two press-release pairs, the second with a click count of two', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('double_click', { x: 1, y: 2 });
    assert.deepEqual(mouse(conn), [
      ['mousePressed', 1, 2, 1],
      ['mouseReleased', 1, 2, 1],
      ['mousePressed', 1, 2, 2],
      ['mouseReleased', 1, 2, 2],
    ]);
  });

  it('double-clicks an element where it sits', async () => {
    const { driver, conn } = pageWithElement();
    await driver.dispatch('double_click', { selector: '#go' });
    assert.deepEqual(mouse(conn)[0], ['mousePressed', 40, 60, 1]);
  });

  it('drags through intermediate moves so movement thresholds fire', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('drag', { from_x: 0, from_y: 0, to_x: 40, to_y: 80 });
    assert.deepEqual(mouse(conn), [
      ['mouseMoved', 0, 0, 1],
      ['mousePressed', 0, 0, 1],
      ...Array.from({ length: DRAG_STEPS }, (_, i) => [
        'mouseMoved',
        (40 * (i + 1)) / DRAG_STEPS,
        (80 * (i + 1)) / DRAG_STEPS,
        1,
      ]),
      ['mouseReleased', 40, 80, 1],
    ]);
  });

  it('hovers over an element or coordinates', async () => {
    const { driver, conn } = pageWithElement();
    await driver.dispatch('hover', { element_id: 2 });
    await driver.dispatch('hover', { x: 9, y: 9 });
    assert.deepEqual(mouse(conn), [
      ['mouseMoved', 40, 60, 1],
      ['mouseMoved', 9, 9, 1],
    ]);
  });
});

describe('wheel scrolling', () => {
  it('scrolls down most of a viewport near the corner by default', async () => {
    const { driver, conn } = pageWithElement();
    assert.deepEqual(await driver.dispatch('scroll-down', {}), { ok: true });
    assert.deepEqual(conn.sent('Input.dispatchMouseEvent')[0].params, {
      type: 'mouseWheel',
      x: WHEEL_ORIGIN,
      y: WHEEL_ORIGIN,
      deltaX: 0,
      deltaY: 500 * SCROLL_PAGE_FRACTION,
    });
  });

  it('scrolls up by the amount asked, at the point given', async () => {
    const { driver, conn } = pageWithElement();
    await driver.dispatch('scroll-up', { amount: 120, x: 3, y: 4 });
    const { x, y, deltaY } = conn.sent('Input.dispatchMouseEvent')[0].params;
    assert.deepEqual([x, y, deltaY], [3, 4, -120]);
  });

  it('assumes a 1280×800 viewport when the page cannot say', async () => {
    const { driver, conn } = fakeDriver();
    await driver.dispatch('scroll-down', {});
    assert.equal(conn.sent('Input.dispatchMouseEvent')[0].params.deltaY, 800 * SCROLL_PAGE_FRACTION);
  });
});
