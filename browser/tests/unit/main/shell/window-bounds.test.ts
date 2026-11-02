/**
 * Unit tests for the main window's saved bounds: what is kept from the file,
 * what is dropped as malformed or off every display, and what is saved on close.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { placement, WindowBounds } from '../../../../src/main/shell/window-bounds.ts';
import { fakeElectron, memoryFile } from '../../support/main-ctx.cjs';

const DISPLAY = [{ x: 0, y: 0, width: 1920, height: 1080 }];
const SAVED = { x: 100, y: 50, width: 1000, height: 700, maximized: false };

describe('placement', () => {
  it('keeps saved bounds that land on a connected display', () => {
    assert.deepEqual(placement(SAVED, DISPLAY), SAVED);
  });

  it('rejects a missing or malformed file', () => {
    for (const bad of [null, 'x', { ...SAVED, x: NaN }, { ...SAVED, width: -5 }, { ...SAVED, maximized: 'yes' }]) {
      assert.equal(placement(bad, DISPLAY), null);
    }
  });

  it('rejects bounds on a display that is no longer connected', () => {
    assert.equal(placement({ ...SAVED, x: 5000 }, DISPLAY), null);
  });

  it('rejects a window that only slivers onto a display', () => {
    assert.equal(placement({ ...SAVED, x: 1910 }, DISPLAY), null);
  });

  it('rejects a window whose title bar would be above the screen', () => {
    assert.equal(placement({ ...SAVED, y: -600, height: 800 }, DISPLAY), null);
  });

  it('clamps a saved size below the minimum up to 600x400', () => {
    const placed = placement({ ...SAVED, width: 200, height: 100 }, DISPLAY);
    assert.deepEqual([placed?.width, placed?.height], [600, 400]);
  });
});

describe('WindowBounds', () => {
  /** A store over memory, and a window built by the fake Electron. */
  function setup() {
    const electron = fakeElectron();
    const file = memoryFile();
    const win = new electron.BrowserWindow();
    new WindowBounds({ electron } as any, file).track(win as any);
    return { file, win };
  }

  it('saves the normal bounds and the maximized flag on close', () => {
    const { file, win } = setup();
    win.normalBounds = { x: 10, y: 20, width: 900, height: 600 };
    win.maximize();
    win.emit('close');
    assert.deepEqual(file.read(), { x: 10, y: 20, width: 900, height: 600, maximized: true });
  });

  it('does not save while fullscreen', () => {
    const { file, win } = setup();
    win.fullScreen = true;
    win.emit('close');
    assert.equal(file.read(), null);
  });

  it('keeps the last good bounds when the window closes minimized', () => {
    const { file, win } = setup();
    win.minimized = true;
    win.emit('close');
    assert.equal(file.read(), null);
  });
});
