/**
 * Unit tests for the source pane's read of a page (page-source.ts): the
 * analyzer is loaded first, the saved page format is used, and nothing
 * renumbers the agent's element ids while it drives.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { PageSource } from '../../../../src/main/tabs/page-source.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';

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
      evaluate: async (_v, expr) => (evaluated.push(expr), expr.includes('outerHTML') ? '<p>' : ANALYSIS),
    };
  });

  it('reads the HTML but not the page while the agent drives, so its element ids survive', async () => {
    ctx.control.state.interactive = false;
    const source = await new PageSource(ctx).active();
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
    const read = ctx.world.evaluate;
    ctx.world.evaluate = async (view, expr) => (
      order.push(expr.includes('analyzePage') ? 'analyze' : 'html'),
      read(view, expr)
    );
    await new PageSource(ctx).active();
    assert.ok(order.indexOf('inject') < order.indexOf('analyze'), order.join(' '));
  });

  it('shows the page in the saved page format', async () => {
    ctx.config.values = { ui: { pageFormat: 'jsonl' } };
    const source = await new PageSource(ctx).active();
    assert.equal(source.markdown, '{"page":{"url":"https://a.test/"}}\n{"region":"main","kind":"h1","text":"Hi"}');
  });
});
