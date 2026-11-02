/**
 * Who may touch the page. While an agent has control, a transparent view (the
 * control shield) sits over the active tab and swallows human input, sign-in
 * popups are disabled, and the page menu items go grey. The shield also shows
 * the agent reading the page: a scan and outlines of what it found, drawn over
 * the page rather than into it, so the site never sees them.
 */
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { HighlightEpoch } from './highlight-epoch.ts';
import { highlightMeasureJs } from './highlight-measure.ts';
import { TakeoverPrompt } from './takeover-prompt.ts';
import type { BrowserView, BrowserWindow, WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { drivenElsewhere, type ControlSnapshot } from '../control/control-state.ts';
import { TRANSPARENT, HUMAN_MENU_ITEMS, SHIELD_TRACK_MS, SHIELD_TRACK_FOR_MS } from './constants.ts';
import { lineFor, namesFrom, idOf, changesPage, type ActionParams, type AnalyzedElement } from './narration.ts';
import { HighlightReader } from './highlight-reader.ts';
import { holdStill, inContainer } from './hold-still.ts';
import { keepAgentRendering } from './view-stack.ts';
import { logShieldPresses } from './shield-press.ts';

/** A closing BrowserView may have already lost its web contents before the page's destruction event. */
function liveContents(view: BrowserView | null): WebContents | null {
  const contents = view?.webContents;
  return contents && !contents.isDestroyed() ? contents : null;
}

/** Carry container motion policy to the shield before its first paint. */
function loadShield(view: BrowserView, appDir: string): void {
  view.webContents.loadFile(path.join(appDir, 'out', 'renderer', 'control-shield', 'index.html'), {
    query: { still: String(inContainer()) },
  });
}

/** The services the shield uses. */
type Deps = Pick<
  AppServices,
  | 'electron'
  | 'appDir'
  | 'control'
  | 'shell'
  | 'recorder'
  | 'shortcuts'
  | 'overlays'
  | 'tabs'
  | 'world'
  | 'agentTab'
  | 'tabHealthLog'
>;

/** What each window's shell was last told about its shown tab. */
const announced = new WeakMap<object, boolean>();

/**
 * The control state as this window's shell reads it, with whether the person
 * may use the tab it shows and open new ones. Every state the shell is ever
 * handed goes through here, the IPC answers included: a raw snapshot lacks the
 * per-tab flags, and the shell then treats agent mode as "nothing allowed".
 */
export function shellState(deps: Pick<Deps, 'agentTab' | 'tabs' | 'shell'>, state: ControlSnapshot): ControlSnapshot {
  const pageInteractive = deps.agentTab.humanMay(deps.tabs.activeTabId);
  announced.set(deps.shell, pageInteractive);
  return { ...state, pageInteractive, newTabInteractive: deps.agentTab.humanMay(null) };
}

/** Tells the shell when the shown tab became the person's or the agent's (a tab switch, a claim, a release). */
function announcePage(deps: Deps): void {
  if (announced.get(deps.shell) === deps.agentTab.humanMay(deps.tabs.activeTabId)) return;
  deps.shell.send('control-state', shellState(deps, deps.control.snapshot()));
}

/** Install shell shortcuts, quiet input blocking and the press log on the shield alone. */
function installShieldInput(deps: Deps, takeover: TakeoverPrompt, view: BrowserView): void {
  deps.shortcuts.install(view.webContents);
  takeover.install(view.webContents);
  logShieldPresses(deps, view.webContents, view);
}

/** Recheck ownership and window identity before restoring focus after a native focus callback. */
function focusFencedShell(deps: Deps, contents: WebContents): void {
  if (contents.isDestroyed() || deps.shell.window?.webContents !== contents) return;
  if (!deps.control.snapshot().interactive) contents.focus();
}

/** Where one element sits, in the shield's pixels, and the kind of element it is. */
export interface ShieldBox {
  /** The element's analysis id. */
  id: number;
  /** Its kind, which colours its outline. */
  type: string;
  /** Visible but unavailable controls keep an outline with a disabled state. */
  disabled?: boolean;
  /** Left edge. */
  x: number;
  /** Top edge. */
  y: number;
  /** Width. */
  w: number;
  /** Height. */
  h: number;
}

/** What an analysis found. */
export interface AnalysisData {
  /** The elements it read, in page order. */
  elements?: AnalyzedElement[];
}

/** An analysis result, as the page reader returns it. */
export interface AnalysisResult {
  /** Whether the analysis ran. */
  ok?: boolean;
  /** What it found. */
  data?: AnalysisData;
}

/** One run of following the outlines; a newer run replaces it. */
interface TrackingRun {
  /** The next re-measure. */
  timer?: NodeJS.Timeout;
}

/** Whether an agent holds the page (`view`'s, under per-tab control): re-analyzing it would renumber the agent's element ids. */
export function agentDriving(ctx: Pick<AppServices, 'control' | 'agentTab' | 'tabs'>, view?: unknown): boolean {
  if (!view || !ctx.agentTab.scoped()) return !ctx.control.snapshot().interactive;
  return ctx.agentTab.claimed().some((id) => ctx.tabs.find(id)?.view === view);
}

/** What a read of the page answers while an agent holds it. */
export const AGENT_HOLDS_PAGE = 'OyaAI is using this page. Switch to Human Mode to read it.';

/** The shield view and the popups it disables. */
export class ControlShield {
  /** The shield's BrowserView, created on first use. */
  view: BrowserView | null = null;
  /** Sign-in popups, each marked whether it is the person's own (opened beside the agent's tab), enabled while they may use it. */
  readonly popups = new Map<BrowserWindow, boolean>();
  /** The outlines being followed right now, or null; a newer run replaces it. */
  private tracking: TrackingRun | null = null;
  /** Rejects geometry measured before a navigation, tab switch or newer action. */
  private readonly epoch = new HighlightEpoch(() => this.reset());
  /** Native geometry reader shared by scans and action feedback. */
  private readonly reader: HighlightReader;
  /** Analysis id to element name, from the last analysis, for the companion's lines. */
  private names: Map<number, string> = new Map();
  /** The main-process services. */
  private readonly deps: Deps;
  /** Input-triggered, deduplicated handoff confirmation. */
  readonly takeover: TakeoverPrompt;

  /** `deps` is the main-process context (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.reader = new HighlightReader(deps, () => this.view);
    this.takeover = new TakeoverPrompt(deps);
  }

  /** Throws unless a person holds control; every page-touching IPC call checks this. */
  requireHumanControl(): void {
    if (!this.deps.control.snapshot().interactive) throw new Error('Switch to Human Mode before using this page');
  }

  /** A tab that takes focus while an agent drives hands it back to the shell. */
  keepFocusOnShell(): void {
    keepShellFocus(this.deps);
  }

  /** The control state changed: tell the shell, and fence the page accordingly. */
  controlChanged(state: ControlSnapshot): void {
    this.deps.shell.send('control-state', shellState(this.deps, state));
    this.deps.recorder.controlLost(state);
    this.enableMenus(state.interactive);
    for (const [popup, own] of this.popups) if (!popup.isDestroyed()) popup.setEnabled(own || !drivenElsewhere(state));
    if (this.deps.shell.alive()) this.sync();
  }

  /** Greys out the page commands in the application menu. */
  enableMenus(interactive: boolean): void {
    updateMenus(this.deps, interactive);
  }

  /** A new sign-in popup: disabled only while someone else drives, unless it is the person's own; forgotten once closed. */
  adoptPopup(childWindow: BrowserWindow, own = false): void {
    this.popups.set(childWindow, own);
    childWindow.setEnabled(own || !drivenElsewhere(this.deps.control.snapshot()));
    childWindow.once('closed', () => this.popups.delete(childWindow));
  }

  /** Creates the shield view, once, and gives it back. */
  prepare(): BrowserView {
    if (this.view) return this.view;
    const webPreferences = { sandbox: true, contextIsolation: true, nodeIntegration: false };
    const view = (this.view = new this.deps.electron.BrowserView({ webPreferences }));
    view.setBackgroundColor(TRANSPARENT);
    void holdStill(view, inContainer());
    loadShield(view, this.deps.appDir);
    installShieldInput(this.deps, this.takeover, view);
    return view;
  }

  /** Whether the page should be covered right now. */
  shouldCover(): boolean {
    const { shell, overlays, agentTab, tabs } = this.deps;
    return shell.browsingMode && !overlays.names.size && !agentTab.humanMay(tabs.activeTabId);
  }

  /**
   * Puts the shield over the active tab, or takes it away. The start page is the
   * shell's own, not a website, so there is nothing there to keep a person's hands
   * off, and its task box must take their typing: it is never covered.
   */
  sync(): void {
    const win: BrowserWindow | null = this.deps.shell.window;
    if (!win || win.isDestroyed()) return;
    announcePage(this.deps);
    const view = this.deps.tabs.getShownView() as BrowserView | null; // a tab shown in the window is a BrowserView; popups have windows of their own
    if (!this.shouldCover() || !view) return this.uncover(win);
    this.cover(win, view, this.prepare());
  }

  /** An agent began reading the page: the shield starts its scan, and stops following the last outlines. */
  analysisStarted(view: BrowserView): void {
    if (!this.showing(view)) return;
    this.stopTracking();
    this.tell(view, { phase: 'scan' });
  }

  /** The analysis is back: outline what it found that the page really shows, then keep the outlines on their elements. A failed measurement outlines nothing. */
  async analysisFinished(view: BrowserView, raw: AnalysisResult | null | undefined): Promise<void> {
    if (!this.showing(view)) return;
    this.stopTracking();
    const ticket = { revision: this.epoch.revision, key: randomUUID() };
    const elements = raw?.data?.elements || [];
    this.names = namesFrom(elements);
    const update = await this.reader.analysis(view, elements, ticket.key);
    if (ticket.revision !== this.epoch.revision || !this.showing(view)) return;
    this.publish(view, update, ticket.key);
  }

  /** Sends current geometry and follows the same captured DOM nodes for the duration of the show. */
  private publish(view: BrowserView, update: object, key: string): void {
    this.tell(view, update);
    this.track(view, highlightMeasureJs([], 'follow', key), Date.now() + SHIELD_TRACK_FOR_MS);
  }

  /** A run from the panel ended: the shield lets go of what it was showing, so nothing claims the agent is still at work. */
  runEnded(): void {
    this.stopTracking();
    const view = this.deps.tabs.getActiveView() as BrowserView | null; // a tab shown in the window is a BrowserView; popups have windows of their own
    if (this.showing(view)) this.tell(view!, { phase: 'end' });
  }

  /** The agent is acting on the page: the companion says what it is doing, and a target ring locks onto the element it acts on. */
  async acting(view: BrowserView, action: string, params?: ActionParams): Promise<void> {
    const text = lineFor(action, params, this.names);
    if (!text || !this.showing(view)) return;
    if (idOf(params) === null) return this.unaimed(view, text, action);
    this.stopTracking();
    const ticket = { revision: this.epoch.revision, key: randomUUID() };
    const box = await this.reader.target(view, idOf(params)!, action, ticket.key);
    if (ticket.revision !== this.epoch.revision || !this.showing(view)) return;
    this.publish(view, { phase: 'act', text, box, changes: changesPage(action) }, ticket.key);
  }

  /** Scrolling keeps following the existing scan; a navigation or untargeted edit invalidates it. */
  private unaimed(view: BrowserView, text: string, action: string): void {
    const changes = changesPage(action);
    if (changes) this.stopTracking();
    this.tell(view, { phase: 'act', text, box: null, changes });
  }

  /** Re-measures every SHIELD_TRACK_MS until `until`, so the outlines stay attached to the page as it scrolls or reflows. */
  private track(view: BrowserView, js: string, until: number): void {
    const run: TrackingRun = (this.tracking = {});
    run.timer = setTimeout(() => this.follow(run, view, js, until), SHIELD_TRACK_MS);
  }

  /** One re-measure of a tracking run; it tells the page, then schedules the next, unless the run was stopped or is over. A page that cannot be read has navigated away: its outlines fade and following stops. */
  private async follow(run: TrackingRun, view: BrowserView, js: string, until: number): Promise<void> {
    if (this.tracking !== run || !this.showing(view) || Date.now() > until) return;
    const boxes = await this.reader.measure(view, js);
    if (this.tracking !== run) return;
    // The page could not be read: it has navigated, so these outlines belong to a page that is gone.
    if (!boxes) return this.tell(view, { phase: 'move', boxes: [] });
    this.tell(view, { phase: 'move', boxes });
    run.timer = setTimeout(() => this.follow(run, view, js, until), SHIELD_TRACK_MS);
  }

  /** Stops following the outlines. */
  stopTracking(): void {
    clearTimeout(this.tracking?.timer);
    this.tracking = null;
    this.epoch.bump();
  }

  /** Removes all geometry immediately when the underlying page is no longer the measured document. */
  private reset(): void {
    this.stopTracking();
    this.names.clear();
    liveContents(this.view)
      ?.executeJavaScript('window.oyaShield?.({"phase":"reset"})')
      .catch(() => {});
  }

  /** Whether the shield is over `view` right now. */
  private showing(view: BrowserView | null | undefined): boolean {
    return !!liveContents(this.view) && this.shouldCover() && this.deps.tabs.getActiveView() === view;
  }

  /** Hands the shield page one update, when it is over `view`. */
  private tell(view: BrowserView, update: object): void {
    if (!this.view || !this.showing(view)) return;
    this.view.webContents.executeJavaScript(`window.oyaShield?.(${JSON.stringify(update)})`).catch(() => {});
  }

  /** Takes the shield off the window. */
  private uncover(win: BrowserWindow): void {
    this.epoch.watch(null);
    this.stopTracking();
    if (this.view) win.removeBrowserView(this.view);
    keepAgentRendering(this.deps, win);
  }

  /** Lays the shield exactly over `view`, on top, and keeps keyboard focus off the page. */
  private cover(win: BrowserWindow, view: BrowserView, shield: BrowserView): void {
    this.epoch.watch(view.webContents);
    shield.setBounds(view.getBounds());
    if (!win.getBrowserViews().includes(shield)) win.addBrowserView(shield);
    win.setTopBrowserView(shield);
    if (view.webContents.isFocused()) win.webContents.focus();
  }
}

/** Restore native focus after Aura completes its current focus callback, with the same ownership fence. */
function keepShellFocus(deps: Deps): void {
  const contents = deps.shell.window?.webContents;
  if (!contents) return;
  focusFencedShell(deps, contents);
  setImmediate(() => focusFencedShell(deps, contents));
}

/** Keeps native page menu commands consistent with the control owner. */
function updateMenus(deps: Deps, interactive: boolean): void {
  for (const id of HUMAN_MENU_ITEMS) {
    const item = deps.electron.Menu.getApplicationMenu()?.getMenuItemById(id);
    if (item) item.enabled = interactive;
  }
}
