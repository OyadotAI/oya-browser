/**
 * Unit tests for the veil's windows as state, without a canvas: they open
 * eased from shut, glide after their element, close when it is gone and open
 * again when it returns, and stop asking for frames at rest.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { RendererConstants as C } from '../../../../src/renderer/core/constants.ts';
import { VeilWindows } from '../../../../src/renderer/control-shield/veil-windows.ts';

const OPEN = C.SHIELD_VEIL_OPEN_MS;
const box = { id: 1, x: 0, y: 0, w: 10, h: 10 };

describe('the veil windows', () => {
  it('opens a window from shut to full, then stops moving', () => {
    const windows = new VeilWindows();
    windows.open(box);
    assert.deepEqual(windows.frame(0), { shapes: [], moving: true });
    const half = windows.frame(OPEN / 2).shapes[0].open;
    assert.ok(half > 0 && half < 1);
    assert.deepEqual(windows.frame(OPEN), { shapes: [{ box: { x: 0, y: 0, w: 10, h: 10 }, open: 1 }], moving: false });
  });

  it('glides a window to where its element moved', () => {
    const windows = new VeilWindows();
    windows.open(box);
    windows.frame(0);
    windows.frame(OPEN);
    windows.move([{ ...box, x: 100 }]);
    windows.frame(OPEN);
    const mid = windows.frame(OPEN + C.SHIELD_VEIL_GLIDE_MS / 2).shapes[0].box.x;
    assert.ok(mid > 0 && mid < 100);
    assert.equal(windows.frame(OPEN + C.SHIELD_VEIL_GLIDE_MS).shapes[0].box.x, 100);
  });

  it('closes a window whose element is gone, and reopens it when it returns', () => {
    const windows = new VeilWindows();
    windows.open(box);
    windows.frame(0);
    windows.frame(OPEN);
    windows.move([]);
    windows.frame(OPEN);
    assert.deepEqual(windows.frame(OPEN * 2), { shapes: [], moving: false });
    windows.move([box]);
    windows.frame(OPEN * 2);
    assert.equal(windows.frame(OPEN * 3).shapes[0].open, 1);
  });

  it('clears every window at once', () => {
    const windows = new VeilWindows();
    windows.open(box);
    windows.clear();
    assert.deepEqual(windows.frame(0), { shapes: [], moving: false });
  });
});
