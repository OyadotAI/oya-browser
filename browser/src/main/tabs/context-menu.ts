/**
 * The right-click menu on a page: navigation, clipboard, and the developer
 * tools (page source and element inspection, shown in the dev panel).
 */
import type { ContextMenuParams, MenuItemConstructorOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { PageView } from '../cdp/cdp.ts';
import { renderedAnalysis } from '../actions/page-format.ts';
import type { ScriptResult } from '../actions/types.ts';
import { agentDriving, AGENT_HOLDS_PAGE } from '../shell/control-shield.ts';
import { PageSource } from './page-source.ts';

/** The services the menu uses. */
type Deps = Pick<
  AppServices,
  'tabs' | 'recorder' | 'electron' | 'shell' | 'layout' | 'protection' | 'world' | 'control' | 'config'
>;

/** The clicked point, as the inspector needs it. */
type Point = Pick<ContextMenuParams, 'x' | 'y'>;

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
export function inspectScript(params: Point): string {
  return INSPECT_TEMPLATE.replace('__OYA_X__', () => String(params.x)).replace('__OYA_Y__', () => String(params.y));
}

/** Copy, Paste, Select All. */
function editItems(params: ContextMenuParams): MenuItemConstructorOptions[] {
  return [
    { label: 'Copy', role: 'copy', enabled: params.editFlags.canCopy },
    { label: 'Paste', role: 'paste', enabled: params.editFlags.canPaste },
    { label: 'Select All', role: 'selectAll' },
    { type: 'separator' },
  ];
}

/** Right-click context menu with DevTools, View Source, Inspect. */
export class ContextMenu {
  /** The tabs, the recording, the dev panel and the analyzer's world. */
  private readonly deps: Deps;
  /** Reads the page for View Page Source. */
  private readonly source: PageSource;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.source = new PageSource(deps);
  }

  /** Shows the menu for a right-click at `params` on `view`'s page. */
  show(view: PageView, params: ContextMenuParams): void {
    const template = [
      ...this.linkItems(params, view),
      ...this.navigationItems(view),
      ...editItems(params),
      ...this.toolItems(view, params),
    ];
    const menu = this.deps.electron.Menu.buildFromTemplate(template);
    menu.popup({ window: this.deps.shell.window ?? undefined });
  }

  /** "Open Link in New Tab", when a link was clicked. */
  private linkItems(params: ContextMenuParams, view: PageView): MenuItemConstructorOptions[] {
    if (!params.linkURL) return [];
    return [
      {
        label: 'Open Link in New Tab',
        click: () => this.deps.tabs.createTab(params.linkURL, true, undefined, view.webContents.session),
      },
      { type: 'separator' },
    ];
  }

  /** Goes back or forward once a recording in progress has the step. */
  private goInHistory(view: PageView, action: 'go_back' | 'go_forward'): () => Promise<unknown> {
    return () =>
      this.deps.recorder
        .recordHistory(action)
        .finally(() => (action === 'go_back' ? view.webContents.goBack() : view.webContents.goForward()));
  }

  /** Back, Forward, Reload. */
  private navigationItems(view: PageView): MenuItemConstructorOptions[] {
    const history = view.webContents.navigationHistory;
    return [
      { label: 'Back', enabled: history.canGoBack(), click: this.goInHistory(view, 'go_back') },
      { label: 'Forward', enabled: history.canGoForward(), click: this.goInHistory(view, 'go_forward') },
      { label: 'Reload', click: () => view.webContents.reload() },
      { type: 'separator' },
    ];
  }

  /** View Page Source, Inspect Element, Open DevTools. */
  private toolItems(view: PageView, params: ContextMenuParams): MenuItemConstructorOptions[] {
    return [
      { label: 'View Page Source', click: () => this.viewPageSource(view) },
      { label: 'Inspect Element', click: () => this.inspectElement(view, params) },
      { label: 'Open DevTools', click: () => view.webContents.openDevTools({ mode: 'detach' }) },
    ];
  }

  /** The page's HTML and markdown, shown in the dev panel's source pane. */
  private async viewPageSource(view: PageView): Promise<void> {
    this.deps.layout.reveal();
    try {
      await this.deps.protection.injectScripts(view);
      this.deps.shell.send('view-source', await this.source.read(view));
    } catch (e) {
      this.deps.shell.send('view-source', { html: '', markdown: '', error: (e as Error).message });
    }
  }

  /** The analyzer's view of the clicked element's section, shown in the dev panel; refused while an agent drives. */
  private async inspectElement(view: PageView, params: Point): Promise<void> {
    this.deps.layout.reveal();
    if (agentDriving(this.deps)) return this.deps.shell.send('inspect-result', { ok: false, error: AGENT_HOLDS_PAGE });
    this.deps.shell.send('inspect-result', await this.inspectRead(view, params));
  }

  /** The analyzer's read of the clicked element's section, or the error it failed with. */
  private async inspectRead(view: PageView, params: Point): Promise<ScriptResult | null> {
    try {
      await this.deps.protection.injectScripts(view);
      return renderedAnalysis(this.deps, await this.deps.world.evaluate(view, inspectScript(params)));
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  }
}
