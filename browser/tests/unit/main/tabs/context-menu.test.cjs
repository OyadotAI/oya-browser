/**
 * Unit tests for the page context menu and the source pane: the items it
 * offers, and what View Source and Inspect send to the dev panel.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { showContextMenu, inspectScript } = require('../../../../main/tabs/context-menu.cjs');
const { activePageSource, pageMarkdown } = require('../../../../main/tabs/page-source.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');

const PARAMS = { x: 10, y: 20, linkURL: '', editFlags: { canCopy: true, canPaste: false } };

/** The template of the menu shown for `params`. */
function menuFor(ctx, view, params) {
  let built;
  ctx.electron.Menu.buildFromTemplate = (template) => (built = { template, popup() {} });
  showContextMenu(ctx, view, params);
  return built.template;
}

describe('context menu', () => {
  let ctx, view;
  beforeEach(() => {
    ctx = mainCtx();
    view = new FakeBrowserView();
    view.webContents.url = 'https://a.test/';
    ctx.world = {
      worldEval: async (_v, expr) => (expr.includes('outerHTML') ? '<html>' : { ok: true, data: { markdown: '# A' } }),
    };
  });

  it('offers a link item only for links', () => {
    const labels = (params) => menuFor(ctx, view, params).map((i) => i.label || i.type);
    assert.equal(labels(PARAMS)[0], 'Back');
    assert.deepEqual(labels({ ...PARAMS, linkURL: 'https://b.test/' }).slice(0, 2), [
      'Open Link in New Tab',
      'separator',
    ]);
  });

  it('mirrors the clipboard state', () => {
    const items = menuFor(ctx, view, PARAMS);
    assert.equal(items.find((i) => i.label === 'Copy').enabled, true);
    assert.equal(items.find((i) => i.label === 'Paste').enabled, false);
  });

  it('shows the page source in the dev panel', async () => {
    menuFor(ctx, view, PARAMS)
      .find((i) => i.label === 'View Page Source')
      .click();
    await flush();
    assert.deepEqual(ctx.shell.sentOn('view-source'), [{ html: '<html>', markdown: '# A', url: 'https://a.test/' }]);
  });

  it('reports a source read that failed', async () => {
    ctx.world.worldEval = async () => {
      throw new Error('detached');
    };
    menuFor(ctx, view, PARAMS)
      .find((i) => i.label === 'View Page Source')
      .click();
    await flush();
    assert.deepEqual(ctx.shell.sentOn('view-source'), [{ html: '', markdown: '', error: 'detached' }]);
  });

  it('inspects the clicked point and shows the result', async () => {
    let script;
    ctx.world.worldEval = async (_v, expr) => ((script = expr), { ok: true });
    menuFor(ctx, view, PARAMS)
      .find((i) => i.label === 'Inspect Element')
      .click();
    await flush();
    assert.match(script, /document\.elementFromPoint\(10, 20\)/);
    assert.deepEqual(ctx.shell.sentOn('inspect-result'), [{ ok: true }]);
  });

  it('builds the inspector with the exact coordinates', () => {
    assert.match(inspectScript({ x: 1.5, y: 2 }), /elementFromPoint\(1\.5, 2\)/);
  });
});

describe('page source', () => {
  it('is empty without an active tab', async () => {
    const ctx = mainCtx();
    ctx.tabs = { getActiveView: () => null };
    assert.deepEqual(await activePageSource(ctx), { html: '', markdown: '', url: '' });
  });

  it('keeps the HTML when the analyzer has no markdown', async () => {
    const ctx = mainCtx();
    ctx.world = { worldEval: async (_v, expr) => (expr.includes('outerHTML') ? '<p>' : null) };
    const view = new FakeBrowserView();
    assert.equal(await pageMarkdown(ctx, view), '');
    ctx.tabs = { getActiveView: () => view };
    assert.deepEqual(await activePageSource(ctx), { html: '<p>', markdown: '', url: 'about:blank' });
  });
});
