/**
 * Unit tests for shell-layout.cjs: where the page sits beside or above the dev
 * panel, at every stage of its animation.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { shellLayout } = require('../../shell-layout.cjs');
const { CHROME_HEIGHT, PANEL_MIN_WIDTH, PANEL_MAX_WIDTH } = require('../../constants.cjs');

describe('shellLayout', () => {
  it('gives the page the whole window below the toolbar when the panel is closed', () => {
    const layout = shellLayout(1280, 860, 0);
    assert.deepEqual(layout.page, { x: 0, y: CHROME_HEIGHT, width: 1280, height: 860 - CHROME_HEIGHT });
    assert.equal(layout.reveal, 0);
    assert.equal(layout.compact, false);
  });

  it('takes the panel width from the side of a wide window when open', () => {
    const layout = shellLayout(1280, 860, 1, 400);
    assert.equal(layout.panelWidth, 400);
    assert.equal(layout.page.width, 880);
  });

  it('keeps the side panel within its bounds and leaves the page room', () => {
    assert.equal(shellLayout(1600, 860, 1, 10).panelWidth, PANEL_MIN_WIDTH);
    assert.equal(shellLayout(1600, 860, 1, 5000).panelWidth, PANEL_MAX_WIDTH);
    assert.equal(shellLayout(1000, 860, 1, 560).panelWidth, 520);
  });

  it('docks the panel at the bottom of a narrow window', () => {
    const layout = shellLayout(800, 600, 1);
    assert.equal(layout.compact, true);
    assert.equal(layout.panelHeight, Math.round((600 - CHROME_HEIGHT) * 0.45));
    assert.equal(layout.page.width, 800);
    assert.equal(layout.page.height, 600 - CHROME_HEIGHT - layout.panelHeight);
  });

  it('reveals the panel in proportion to the animation progress', () => {
    const layout = shellLayout(1280, 860, 0.5, 400);
    assert.equal(layout.progress, 0.5);
    assert.equal(layout.reveal, 200);
  });

  it('never gives the page less than one pixel', () => {
    const layout = shellLayout(0, 0, 1);
    assert.equal(layout.page.width, 1);
    assert.equal(layout.page.height, 1);
  });
});
