/**
 * Unit tests for CDP mouse input: paths start where the pointer was left and
 * end on the target, clicks press and release there, and wheel events pass
 * their deltas through.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import * as c from '../../../../src/main/input/constants.ts';
import { Mouse } from '../../../../src/main/input/mouse.ts';
import { instantTimers, fixedRandom, pageView, mouseEvents } from '../../support/page.cjs';

describe('CDP mouse', () => {
  afterEach(() => mock.restoreAll());

  it('a move eases along a path that ends on the target', async () => {
    instantTimers();
    fixedRandom(0.5);
    const view = pageView();
    await new Mouse().move(view, 300, 400);
    const moves = mouseEvents(view);
    assert.ok(moves.every((e: any) => e.type === 'mouseMoved'));
    assert.deepEqual(moves.at(-1), { type: 'mouseMoved', x: 300, y: 400 });
    assert.deepEqual(moves[0], { type: 'mouseMoved', x: 0, y: 0 });
  });

  it('path length grows with distance, within its bounds', async () => {
    instantTimers();
    fixedRandom(0.5);
    const mouse = new Mouse();
    const short = pageView();
    await mouse.move(short, 1, 1);
    const long = pageView();
    await mouse.move(long, 5000, 1);
    assert.equal(mouseEvents(short).length, c.MIN_PATH_STEPS + 1);
    assert.equal(mouseEvents(long).length, c.MAX_PATH_STEPS + 1);
  });

  it('the next move starts where the last one ended', async () => {
    instantTimers();
    fixedRandom(0.5);
    const mouse = new Mouse();
    await mouse.move(pageView(), 50, 60);
    const view = pageView();
    await mouse.move(view, 50, 60);
    assert.ok(mouseEvents(view).every((e: any) => e.x === 50 && e.y === 60));
  });

  it('a click rounds its point and presses then releases the left button', async () => {
    const delays = instantTimers();
    fixedRandom(0);
    const view = pageView();
    await new Mouse().click(view, 10.4, 20.6);
    const [pressed, released] = mouseEvents(view).slice(-2);
    assert.deepEqual(pressed, { type: 'mousePressed', x: 10, y: 21, button: 'left', clickCount: 1, buttons: 1 });
    assert.deepEqual(released, { type: 'mouseReleased', x: 10, y: 21, button: 'left', clickCount: 1 });
    assert.deepEqual(delays.slice(-2), [c.CLICK_PAUSE.base, c.CLICK_PAUSE.base]);
  });

  it('a scroll is one wheel event', async () => {
    const view = pageView();
    await new Mouse().scroll(view, 1, 2, 3, 4);
    assert.deepEqual(mouseEvents(view), [{ type: 'mouseWheel', x: 1, y: 2, deltaX: 3, deltaY: 4 }]);
  });
});
