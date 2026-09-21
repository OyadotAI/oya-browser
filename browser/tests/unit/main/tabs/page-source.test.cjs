/**
 * Unit tests for the source pane's read of a page (page-source.cjs): the
 * analyzer is loaded first, the saved page format is used, and nothing
 * renumbers the agent's element ids while it drives.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { activePageSource } = require('../../../../main/tabs/page-source.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');

/** An analysis the analyzer could answer. */
const ANALYSIS = {
  ok: true,
  data: { facts: { url: 'https://a.test/' }, blocks: [{ region: 'main', kind: 'h1', text: 'Hi' }] },
};

describe('page source', () => {
  let ctx, view, evaluated;
  beforeEach(() => {
    ctx = mainCtx();
    view = new FakeBrowserView();
    view.webContents.url = 'https://a.test/';
    ctx.tabs = { getActiveView: () => view };
    evaluated = [];
    ctx.world = {
      worldEval: async (_v, expr) => (evaluated.push(expr), expr.includes('outerHTML') ? '<p>' : ANALYSIS),
    };
  });

  it('reads the HTML but not the page while the agent drives, so its element ids survive', async () => {
    ctx.control.state.interactive = false;
    const source = await activePageSource(ctx);
    assert.equal(source.html, '<p>');
    assert.match(source.notice, /Take control/);
    assert.equal(
      evaluated.some((e) => e.includes('analyzePage')),
      false,
    );
  });

  it('loads the analyzer before analyzing the page', async () => {
    const order = [];
    ctx.protection.injectScripts = async () => order.push('inject');
    const read = ctx.world.worldEval;
    ctx.world.worldEval = async (view, expr) => (
      order.push(expr.includes('analyzePage') ? 'analyze' : 'html'),
      read(view, expr)
    );
    await activePageSource(ctx);
    assert.ok(order.indexOf('inject') < order.indexOf('analyze'), order.join(' '));
  });

  it('shows the page in the saved page format', async () => {
    ctx.config.values = { ui: { pageFormat: 'jsonl' } };
    const source = await activePageSource(ctx);
    assert.equal(source.markdown, '{"page":{"url":"https://a.test/"}}\n{"region":"main","kind":"h1","text":"Hi"}');
  });
});
