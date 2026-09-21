/**
 * The right-click menu on a page: navigation, clipboard, and the developer
 * tools (page source and element inspection, shown in the dev panel).
 */
const { renderedAnalysis } = require('../page-format.cjs');
const { readPageSource } = require('./page-source.cjs');
const { agentDriving, AGENT_HOLDS_PAGE } = require('../shell/control-shield.cjs');

/**
 * The inspector, run in the analyzer's isolated world at the clicked point.
 * This text runs in the page, so it changes only on purpose; it no longer asks
 * for in-page labels, which nothing ever cleaned up. __OYA_X__ and __OYA_Y__
 * are the clicked point.
 */
const INSPECT_TEMPLATE = `
            (function() {
              const el = document.elementFromPoint(__OYA_X__, __OYA_Y__);
              if (!el) return { ok: false, error: 'No element at coordinates' };
              // Walk up to find nearest element with data-ac-id, or use the element itself
              let target = el;
              while (target && !target.getAttribute('data-ac-id') && target !== document.body) {
                target = target.parentElement;
              }
              // Run analyzePage scoped to this element's parent section
              if (typeof analyzePage === 'function') {
                // Find a reasonable scope, the element's closest section/article/main or its parent
                let scope = el.closest('section, article, main, [role="main"], [role="dialog"], form, nav, aside') || el.parentElement || el;
                // Generate a unique temporary selector
                const tmpId = '__oya_inspect_' + Date.now();
                scope.setAttribute('data-oya-inspect', tmpId);
                const result = analyzePage({ selector: '[data-oya-inspect="' + tmpId + '"]' });
                scope.removeAttribute('data-oya-inspect');
                return result;
              }
              return { ok: false, error: 'Analyzer not loaded' };
            })()
          `;

/** The inspector for the point the person right-clicked. */
function inspectScript(params) {
  return INSPECT_TEMPLATE.replace('__OYA_X__', () => String(params.x)).replace('__OYA_Y__', () => String(params.y));
}

/** "Open Link in New Tab", when a link was clicked. */
function linkItems(ctx, params) {
  if (!params.linkURL) return [];
  return [
    { label: 'Open Link in New Tab', click: () => ctx.tabs.createTab(params.linkURL, true) },
    { type: 'separator' },
  ];
}

/** Back, Forward, Reload. */
function navigationItems(view) {
  const history = view.webContents.navigationHistory;
  return [
    { label: 'Back', enabled: history.canGoBack(), click: () => view.webContents.goBack() },
    { label: 'Forward', enabled: history.canGoForward(), click: () => view.webContents.goForward() },
    { label: 'Reload', click: () => view.webContents.reload() },
    { type: 'separator' },
  ];
}

/** Copy, Paste, Select All. */
function editItems(params) {
  return [
    { label: 'Copy', role: 'copy', enabled: params.editFlags.canCopy },
    { label: 'Paste', role: 'paste', enabled: params.editFlags.canPaste },
    { label: 'Select All', role: 'selectAll' },
    { type: 'separator' },
  ];
}

/** View Page Source, Inspect Element, Open DevTools. */
function toolItems(ctx, view, params) {
  return [
    { label: 'View Page Source', click: () => viewPageSource(ctx, view) },
    { label: 'Inspect Element', click: () => inspectElement(ctx, view, params) },
    { label: 'Open DevTools', click: () => view.webContents.openDevTools({ mode: 'detach' }) },
  ];
}

/** Right-click context menu with DevTools, View Source, Inspect. */
function showContextMenu(ctx, view, params) {
  const template = [
    ...linkItems(ctx, params),
    ...navigationItems(view),
    ...editItems(params),
    ...toolItems(ctx, view, params),
  ];
  const menu = ctx.electron.Menu.buildFromTemplate(template);
  menu.popup({ window: ctx.shell.window });
}

/** The page's HTML and markdown, shown in the dev panel's source pane. */
async function viewPageSource(ctx, view) {
  ctx.layout.reveal();
  try {
    await ctx.protection.injectScripts(view);
    ctx.shell.send('view-source', await readPageSource(ctx, view));
  } catch (e) {
    ctx.shell.send('view-source', { html: '', markdown: '', error: e.message });
  }
}

/** The analyzer's view of the clicked element's section, shown in the dev panel; refused while an agent drives. */
async function inspectElement(ctx, view, params) {
  ctx.layout.reveal();
  if (agentDriving(ctx)) return ctx.shell.send('inspect-result', { ok: false, error: AGENT_HOLDS_PAGE });
  ctx.shell.send('inspect-result', await inspectRead(ctx, view, params));
}

/** The analyzer's read of the clicked element's section, or the error it failed with. */
async function inspectRead(ctx, view, params) {
  try {
    await ctx.protection.injectScripts(view);
    return renderedAnalysis(ctx, await ctx.world.worldEval(view, inspectScript(params), true));
  } catch (e) {
    return { ok: false, error: e.message };
  }
}

module.exports = { showContextMenu, inspectScript };
