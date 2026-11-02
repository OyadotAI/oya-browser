/** A tab joins another window only when let go over its tab strip, on any monitor. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { inTabStrip, inWindow } from '../../../../src/main/windows/geometry.ts';
import { WINDOW_TAB_STRIP_HEIGHT } from '../../../../src/main/windows/constants.ts';

/** A window on a monitor left of the main one. */
const LEFT = { x: -1600, y: 0, width: 1280, height: 800 };

it('counts the strip of a window on a negative-coordinate monitor', () => {
  assert.equal(inTabStrip({ x: -1500, y: 10 }, LEFT), true);
});

it('does not count the page below the strip as the strip', () => {
  assert.equal(inTabStrip({ x: -1500, y: WINDOW_TAB_STRIP_HEIGHT }, LEFT), false);
  assert.equal(inWindow({ x: -1500, y: WINDOW_TAB_STRIP_HEIGHT }, LEFT), true);
});
