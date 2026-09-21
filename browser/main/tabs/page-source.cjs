/**
 * A page as the dev panel's source pane shows it: the raw HTML, and the page as
 * the analyzer reads it, in the configured format (markdown unless set).
 */
const { renderedAnalysis } = require('../page-format.cjs');
const { agentDriving, AGENT_HOLDS_PAGE } = require('../shell/control-shield.cjs');

/**
 * The page as the analyzer reads it, rendered, with the analysis kept to render
 * again; empty when it cannot say. Not while an agent drives: analyzing resets
 * the element ids the agent's next click relies on.
 */
async function pageRead(ctx, view) {
  if (agentDriving(ctx)) return { markdown: '', analysis: null, notice: AGENT_HOLDS_PAGE };
  try {
    await ctx.protection.injectScripts(view);
    const raw = await ctx.world.worldEval(view, '(typeof analyzePage === "function") ? analyzePage({}) : null');
    const result = renderedAnalysis(ctx, raw);
    if (result?.ok) return { markdown: result.data.page || result.data.markdown || '', analysis: result.data };
  } catch {}
  return { markdown: '', analysis: null };
}

/** The analyzer's page for the page, or '' when it cannot say. */
const pageMarkdown = async (ctx, view) => (await pageRead(ctx, view)).markdown;

/** A page's HTML, its read (rendered, and the analysis behind it) and address. */
async function readPageSource(ctx, view) {
  const html = await ctx.world.worldEval(view, 'document.documentElement.outerHTML');
  return { html, ...(await pageRead(ctx, view)), url: view.webContents.getURL() };
}

/** The active page's HTML, markdown and address; empty fields when there is none. */
async function activePageSource(ctx) {
  const view = ctx.tabs.getActiveView();
  if (!view) return { html: '', markdown: '', url: '' };
  try {
    return await readPageSource(ctx, view);
  } catch (e) {
    return { html: '', markdown: '', url: '', error: e.message };
  }
}

module.exports = { pageMarkdown, readPageSource, activePageSource };
