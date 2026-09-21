/**
 * Unit tests for Overlays: the page frozen as a backdrop, the bounded wait
 * for the shell to paint it, and the page restored when the last one goes.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { Overlays } = require('../../../../main/shell/overlays.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');

describe('Overlays', () => {
  let ctx, page;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    ctx = mainCtx({ overlays: Overlays });
    page = new FakeBrowserView();
    ctx.tabs = { getActiveView: () => page };
    ctx.shell.window.setBrowserView(page);
  });
  afterEach(() => mock.timers.reset());

  it('freezes the page as a backdrop, then takes the view away once painted', async () => {
    const shown = ctx.overlays.show('shell');
    await flush();
    const [backdrop] = ctx.shell.sentOn('page-backdrop');
    assert.equal(backdrop.image, 'data:image/png;base64,AA');
    ctx.overlays.backdropReady(backdrop.token);
    await shown;
    assert.deepEqual(ctx.shell.window.views, []);
    assert.ok(ctx.overlays.names.has('shell'));
  });

  it('stops waiting for the backdrop after a short while', async () => {
    const shown = ctx.overlays.show();
    await flush();
    mock.timers.tick(300);
    await shown;
    assert.ok(ctx.overlays.names.has('legacy'));
    assert.equal(ctx.overlays.backdropWaiters.size, 0);
  });

  it('ignores an overlay name it does not know', async () => {
    await ctx.overlays.show('evil');
    assert.equal(ctx.overlays.names.size, 0);
  });

  it('carries on when the page cannot be captured', async () => {
    page.webContents.capturePage = async () => {
      throw new Error('crashed');
    };
    await ctx.overlays.show('shell');
    assert.ok(ctx.overlays.names.has('shell'));
  });

  it('restores the page and clears the backdrop when the last overlay goes', () => {
    ctx.overlays.names.add('shell').add('legacy');
    ctx.shell.window.setBrowserView(null);
    ctx.overlays.hide('shell');
    assert.deepEqual(ctx.shell.window.views, []);
    ctx.overlays.hide();
    assert.deepEqual(ctx.shell.window.views, [page]);
    assert.deepEqual(ctx.shell.sentOn('page-backdrop'), [null]);
  });
});
