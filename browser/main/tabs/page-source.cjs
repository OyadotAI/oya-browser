/**
 * A page as the dev panel's source pane shows it: the raw HTML, and the
 * markdown the analyzer reads the page as.
 */

/** The analyzer's markdown for the page, or '' when it cannot say. */
async function pageMarkdown(ctx, view) {
  try {
    const result = await ctx.world.worldEval(view, '(typeof analyzePage === "function") ? analyzePage({}) : null');
    if (result?.ok) return result.data.markdown || '';
  } catch {}
  return '';
}

/** A page's HTML, markdown and address. */
async function readPageSource(ctx, view) {
  const html = await ctx.world.worldEval(view, 'document.documentElement.outerHTML');
  const markdown = await pageMarkdown(ctx, view);
  return { html, markdown, url: view.webContents.getURL() };
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
