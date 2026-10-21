/**
 * Unit tests for PanelLayout: page bounds, the eased slide, the clamped and
 * debounced width, and the flush on quit.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PanelLayout } from '../../../../src/main/shell/layout.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';
import { shellLayout } from '../../../../src/main/shell/shell-layout.ts';
import { BOOKMARKS_BAR_HEIGHT, CHROME_HEIGHT, FIND_BAR_HEIGHT } from '../../../../src/main/shell/constants.ts';

describe('PanelLayout', () => {
  let ctx: any, view: any;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
    ctx = mainCtx({ layout: PanelLayout });
    view = new FakeBrowserView();
    ctx.tabs = { getActiveView: () => view };
  });
  afterEach(() => mock.timers.reset());

  it('places the page below the toolbar and the bookmarks bar, beside the closed panel', () => {
    const top = CHROME_HEIGHT + BOOKMARKS_BAR_HEIGHT;
    ctx.layout.layoutActiveTab();
    assert.deepEqual(view.bounds, shellLayout(1280, 800, 0, 360, top).page);
    assert.deepEqual(ctx.shell.sentOn('shell-layout')[0], { ...shellLayout(1280, 800, 0, 360, top), panelOpen: false });
  });

  it("gives the bookmarks bar's rows back to the page when the person hides it", () => {
    ctx.config.values.ui = { bookmarksBar: false };
    ctx.layout.layoutActiveTab();
    assert.equal(view.bounds.y, CHROME_HEIGHT);
    assert.equal(ctx.shell.sentOn('shell-layout')[0].chromeHeight, CHROME_HEIGHT);
  });

  it('moves the page down by the find bar while it is open on the shown tab', () => {
    ctx.findBar.height = () => FIND_BAR_HEIGHT;
    ctx.layout.layoutActiveTab();
    assert.equal(view.bounds.y, CHROME_HEIGHT + BOOKMARKS_BAR_HEIGHT + FIND_BAR_HEIGHT);
    assert.equal(
      ctx.shell.sentOn('shell-layout')[0].chromeHeight,
      CHROME_HEIGHT + BOOKMARKS_BAR_HEIGHT + FIND_BAR_HEIGHT,
    );
  });

  it('replays the open interaction state with geometry after shell load or resize', () => {
    ctx.layout.reveal();
    ctx.layout.layoutActiveTab();
    assert.equal(ctx.shell.sentOn('shell-layout').at(-1).panelOpen, true);
  });

  it('closing disables interaction while the panel is still animating onscreen', () => {
    ctx.layout.reveal();
    ctx.layout.toggle();
    const snapshot = ctx.shell.sentOn('shell-layout').at(-1);
    assert.equal(snapshot.panelOpen, false);
    assert.ok(snapshot.progress > 0);
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

  it('shuts the panel on every tab at sign-out and says so, with no slide left running', () => {
    ctx.tabs.activeTabId = 1;
    ctx.layout.toggle();
    ctx.tabs.activeTabId = 2;
    ctx.layout.toggle(true);
    ctx.layout.closeAll();
    assert.equal(ctx.layout.open, false);
    assert.equal(ctx.layout.progress, 0);
    ctx.tabs.activeTabId = 1;
    assert.equal(ctx.layout.open, false);
    assert.equal(ctx.shell.sentOn('dev-panel-state').at(-1), false);
  });

  it('a site tab opened from one with the panel open keeps it open; a start page stays shut', () => {
    ctx.tabs.activeTabId = 1;
    ctx.layout.toggle(true);
    ctx.layout.inherit(2, 'https://a.test/');
    ctx.layout.inherit(3, 'oya:home');
    ctx.tabs.activeTabId = 2;
    assert.equal(ctx.layout.open, true);
    ctx.tabs.activeTabId = 3;
    assert.equal(ctx.layout.open, false);
  });

  it('keeps the panel open or shut per tab, so closing it on one tab leaves the others', () => {
    ctx.tabs.activeTabId = 1;
    ctx.layout.toggle(true);
    ctx.tabs.activeTabId = 2;
    ctx.layout.toggle(true);
    ctx.layout.toggle(true);
    assert.equal(ctx.layout.open, false);
    ctx.tabs.activeTabId = 1;
    assert.equal(ctx.layout.open, true);
  });
});
