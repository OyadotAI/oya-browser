/**
 * Find in page (Cmd/Ctrl+F): one window's find bar over its shown tab. It runs on
 * Electron's stock findInPage, so nothing is injected into the page and no debugger
 * is used. The bar belongs to one tab for one session: a tab switch, a new page or
 * the agent taking the tab closes it, so a count never outlives the page it counted.
 */
import type { Result, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { FindBarState } from '../../shared/ipc.ts';
import { shownViewOf } from '../tabs/home.ts';
import { FIND_BAR_HEIGHT, FIND_QUERY_MAX_CHARS } from './constants.ts';

/** The services the find bar uses. */
type Deps = Pick<AppServices, 'tabs' | 'shell' | 'layout' | 'agentTab'>;

/** What the shell is told when the bar shuts. */
const CLOSED: FindBarState = { open: false, active: 0, total: null };
/** A request id no reply carries: replies to anything sent before it are dropped. */
const NO_REQUEST = -1;

/** One window's find bar: which tab it searches, its query, and the latest request it waits on. */
export class FindBar {
  /** The tab the bar is open on, or null while it is shut. */
  private tabId: number | null = null;
  /** That tab's page. */
  private contents: WebContents | null = null;
  /** What the person is looking for; empty until they type. */
  private query = '';
  /** The latest findInPage request: only its reply may set the count. */
  private requestId = NO_REQUEST;
  /** Removes the listeners this session put on the page. */
  private detach: () => void = () => {};
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is this window's scope (see src/main/windows/context.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Cmd/Ctrl+F: opens the bar on the shown page, or hands the key to a shell page (the library) that has its own search. */
  open(): void {
    const id = this.deps.tabs.activeTabId;
    const tab = this.deps.tabs.find(id);
    if (!tab || tab.window || !this.deps.agentTab.humanMay(id)) return;
    const view = shownViewOf(tab);
    if (view && this.tabId !== tab.id) this.attach(tab.id, view.webContents as WebContents);
    this.deps.shell.window?.webContents.focus();
    this.deps.shell.send('shell-command', 'find');
  }

  /** The person typed: a new query starts a new search; an empty one clears the highlights. */
  search(text: unknown): void {
    if (typeof text !== 'string' || !this.usable()) return;
    // Too long to be on a page: end the old search rather than leave its count and steps running.
    this.query = text.length > FIND_QUERY_MAX_CHARS ? '' : text;
    if (!this.query) return this.clear();
    this.requestId = this.contents!.findInPage(this.query, { findNext: true });
  }

  /** Enter, Shift+Enter, Cmd/Ctrl+G: the next or previous match of the current query. */
  step(forward: boolean): void {
    if (!this.query || !this.usable()) return;
    this.requestId = this.contents!.findInPage(this.query, { forward, findNext: false });
  }

  /** Shuts the bar and clears its highlights; the one way out, whatever closed it. */
  close(): void {
    const contents = this.contents;
    if (!contents) return;
    this.detach();
    Object.assign(this, { tabId: null, contents: null, query: '', requestId: NO_REQUEST, detach: () => {} });
    if (!contents.isDestroyed()) contents.stopFindInPage('clearSelection');
    // A window being torn down has no page to lay out.
    if (this.deps.shell.alive()) this.deps.layout.layoutActiveTab();
    this.deps.shell.send('find-bar', CLOSED);
  }

  /** Esc or the close button: shuts the bar and gives the keyboard back to the page. */
  dismiss(): void {
    const contents = this.contents;
    this.close();
    if (contents && !contents.isDestroyed()) contents.focus();
  }

  /** Another tab is shown: the bar does not follow it. */
  follow(activeId: number | null): void {
    if (this.tabId !== null && this.tabId !== activeId) this.close();
  }

  /** Control changed hands: the bar shuts once the person may no longer use its tab. */
  sync(): void {
    if (this.tabId !== null && !this.deps.agentTab.humanMay(this.tabId)) this.close();
  }

  /** How much room the bar takes above the shown page. */
  height(): number {
    return this.tabId !== null && this.tabId === this.deps.tabs.activeTabId ? FIND_BAR_HEIGHT : 0;
  }

  /** Whether a query may run now: the bar is open and its tab is still the person's. */
  private usable(): boolean {
    return !!this.contents && !this.contents.isDestroyed() && this.deps.agentTab.humanMay(this.tabId);
  }

  /** Opens the bar on tab `id`, listening to its page for this session only. */
  private attach(id: number, contents: WebContents): void {
    this.close();
    this.detach = this.listen(contents);
    Object.assign(this, { tabId: id, contents });
    this.deps.layout.layoutActiveTab();
    this.deps.shell.send('find-bar', { open: true, active: 0, total: null } satisfies FindBarState);
  }

  /** Counts from `contents`, and closes when it loads a new page or goes away; answers how to stop listening. */
  private listen(contents: WebContents): () => void {
    const found = (_event: unknown, result: Result): void => this.found(result);
    const gone = (): void => this.close();
    contents.on('found-in-page', found).on('did-navigate', gone).on('destroyed', gone);
    return () => void contents.off('found-in-page', found).off('did-navigate', gone).off('destroyed', gone);
  }

  /** A reply from the page: only the final one for the latest request sets the count. */
  private found(result: Result): void {
    if (result.requestId !== this.requestId || !result.finalUpdate) return;
    this.deps.shell.send('find-bar', { open: true, active: result.activeMatchOrdinal, total: result.matches });
  }

  /** The query was emptied: no highlights, no count, and late replies ignored. */
  private clear(): void {
    this.requestId = NO_REQUEST;
    this.contents!.stopFindInPage('clearSelection');
    this.deps.shell.send('find-bar', { open: true, active: 0, total: null } satisfies FindBarState);
  }
}
