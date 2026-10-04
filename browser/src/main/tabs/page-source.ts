/**
 * A page as the dev panel's source pane shows it: the raw HTML, and the page as
 * the analyzer reads it, in the configured format (markdown unless set).
 */
import type { AppServices } from '../app/services.ts';
import type { PageView } from '../cdp/cdp.ts';
import { renderedAnalysis } from '../actions/page-format.ts';
import { agentDriving, AGENT_HOLDS_PAGE } from '../shell/control-shield.ts';

/** The services the source pane reads through. */
type Deps = Pick<AppServices, 'tabs' | 'control' | 'protection' | 'world' | 'config'>;

/** The page as the analyzer reads it: rendered, the analysis behind it, and why it is empty when it is. */
export interface PageRead {
  /** The page in the saved format; '' when it cannot say. */
  markdown: string;
  /** The analysis, kept to render again; null when there is none. */
  analysis: unknown;
  /** Why the page was not read, when an agent holds it. */
  notice?: string;
}

/** What the source pane shows: the HTML, the read, the address, and an error when reading failed. */
export interface PageSourceView extends Partial<PageRead> {
  /** The page's HTML. */
  html: unknown;
  /** The page's address. */
  url?: string;
  /** Why it could not be read. */
  error?: string;
}

/** What an analysis answers, as far as the pane uses it. */
interface RenderedPage {
  /** The page in the chosen format. */
  page?: string;
  /** The page as markdown, from an analyzer that names it so. */
  markdown?: string;
}

/** Reads pages for the dev panel's source pane. */
export class PageSource {
  /** The tabs, who drives, the analyzer's world and the saved page format. */
  private readonly deps: Deps;

  /** `deps` are the main-process services (see services.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /**
   * The page as the analyzer reads it, rendered, with the analysis kept to render
   * again; empty when it cannot say. Not while an agent drives: analyzing resets
   * the element ids the agent's next click relies on.
   */
  async pageRead(view: PageView): Promise<PageRead> {
    if (agentDriving(this.deps)) return { markdown: '', analysis: null, notice: AGENT_HOLDS_PAGE };
    try {
      const data = await this.analyze(view);
      if (data) return { markdown: data.page || data.markdown || '', analysis: data };
    } catch {}
    return { markdown: '', analysis: null };
  }

  /** The analyzer's read of the page, rendered in the saved format; undefined when it did not answer one. */
  private async analyze(view: PageView): Promise<RenderedPage | undefined> {
    await this.deps.protection.injectScripts(view);
    const raw = await this.deps.world.evaluate(view, '(typeof analyzePage === "function") ? analyzePage({}) : null');
    const result = renderedAnalysis(this.deps, raw);
    return result?.ok ? (result.data as RenderedPage) : undefined;
  }

  /** The analyzer's page for the page, or '' when it cannot say. */
  async pageMarkdown(view: PageView): Promise<string> {
    return (await this.pageRead(view)).markdown;
  }

  /** A page's HTML, its read (rendered, and the analysis behind it) and address. */
  async read(view: PageView): Promise<PageSourceView> {
    const html: unknown = await this.deps.world.evaluate(view, 'document.documentElement.outerHTML');
    return { html, ...(await this.pageRead(view)), url: view.webContents.getURL() };
  }

  /** The active page's HTML, markdown and address; empty fields when there is none. */
  async active(): Promise<PageSourceView> {
    const view = this.deps.tabs.getActiveView();
    if (!view) return { html: '', markdown: '', url: '' };
    try {
      return await this.read(view);
    } catch (e) {
      return { html: '', markdown: '', url: '', error: (e as Error).message };
    }
  }
}
