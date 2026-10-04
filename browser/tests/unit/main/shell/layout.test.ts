/**
 * Unit tests for PanelLayout: page bounds, the eased slide, the clamped and
 * debounced width, and the flush on quit.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PanelLayout } from '../../../../src/main/shell/layout.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';
import { shellLayout } from '../../../../src/main/shell/shell-layout.ts';

describe('PanelLayout', () => {
  let ctx: any, view: any;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
    ctx = mainCtx({ layout: PanelLayout });
    view = new FakeBrowserView();
    ctx.tabs = { getActiveView: () => view };
  });
  afterEach(() => mock.timers.reset());

  it('places the page below the toolbar, beside the closed panel', () => {
    ctx.layout.layoutActiveTab();
    assert.deepEqual(view.bounds, shellLayout(1280, 800, 0, 360).page);
    assert.deepEqual(ctx.shell.sentOn('shell-layout')[0], shellLayout(1280, 800, 0, 360));
  });

  it('leaves the page alone outside browsing mode', () => {
    ctx.shell.browsingMode = false;
    ctx.layout.layoutActiveTab();
    assert.deepEqual(view.bounds, { x: 0, y: 0, width: 0, height: 0 });
  });

  it('opens at once when the person prefers reduced motion', () => {
    assert.equal(ctx.layout.toggle(true), true);
    assert.equal(ctx.layout.progress, 1);
    assert.deepEqual(ctx.shell.sentOn('dev-panel-state'), [true]);
  });

  it('slides open over the motion time and settles fully open', () => {
    const clock = mock.method(performance, 'now', () => 0);
    ctx.layout.toggle();
    assert.equal(ctx.layout.progress, 0);
    clock.mock.mockImplementation(() => 110);
    mock.timers.tick(16);
    assert.ok(ctx.layout.progress > 0.5 && ctx.layout.progress < 1, 'eased past halfway at half time');
    clock.mock.mockImplementation(() => 500);
    mock.timers.tick(16);
    assert.equal(ctx.layout.progress, 1);
  });

  it('stops sliding once the window is gone', () => {
    mock.method(performance, 'now', () => 0);
    ctx.layout.toggle();
    ctx.shell.window.destroyed = true;
    const sent = ctx.shell.sent.length;
    mock.timers.tick(160);
    assert.equal(ctx.shell.sent.length, sent);
  });

  it('clamps the width, saves it once the drag settles, and ignores non-numbers', () => {
    assert.equal(ctx.layout.resize(9999), 560);
    assert.equal(ctx.layout.resize(10), 320);
    assert.equal(ctx.layout.resize(Number.NaN), 320);
    assert.equal(ctx.config.saves, 0);
    mock.timers.tick(180);
    assert.equal(ctx.config.saves, 1);
    assert.equal(ctx.config.values.ui.panelWidth, 320);
  });

  it('writes an unsaved width on quit', () => {
    ctx.layout.resize(400);
    ctx.layout.flush();
    assert.equal(ctx.config.saves, 1);
    mock.timers.tick(1000);
    assert.equal(ctx.config.saves, 1);
  });

  it('reveals the panel once for a result, without a slide', () => {
    ctx.layout.reveal();
    ctx.layout.reveal();
    assert.equal(ctx.layout.open, true);
    assert.deepEqual(ctx.shell.sentOn('dev-panel-state'), [true]);
  });
});
