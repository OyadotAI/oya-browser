/**
 * The format an analysis's page is written in, decided in one place: the
 * caller's (a server command's `format`), else the one chosen in settings
 * (`ui.pageFormat`), else markdown. The renderers are scripts/page-render.cjs.
 */
const { withPage } = require('../scripts/page-render.cjs');

/** An analyze result with its page rendered in the format that applies. */
function renderedAnalysis(ctx, result, params) {
  return withPage(result, params?.format || ctx.config?.values?.ui?.pageFormat);
}

module.exports = { renderedAnalysis };
