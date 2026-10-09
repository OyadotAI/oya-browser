/**
 * Who may touch the page. While an agent has control, a transparent view (the
 * control shield) sits over the active tab and swallows human input, sign-in
 * popups are disabled, and the page menu items go grey. The shield also shows
 * the agent reading the page: a scan and outlines of what it found, drawn over
 * the page rather than into it, so the site never sees them.
 */
import path from 'node:path';
import { TakeoverPrompt } from './takeover-prompt.ts';
import type { BrowserView, BrowserWindow } from 'electron';
import type { AppServices } from '../app/services.ts';
import { drivenElsewhere, type ControlSnapshot } from '../control/control-state.ts';
import {
  TRANSPARENT,
  HUMAN_MENU_ITEMS,
  MAX_ANALYSIS_BOXES,
  SHIELD_TRACK_MS,
  SHIELD_TRACK_FOR_MS,
} from './constants.ts';
import { lineFor, namesFrom, idOf, changesPage, type ActionParams, type AnalyzedElement } from './narration.ts';
import { pageTone } from './page-tone.ts';
import { holdStill, inContainer } from './hold-still.ts';

/** Carry container motion policy to the shield before its first paint. */
function loadShield(view: BrowserView, appDir: string): void {
  view.webContents.loadFile(path.join(appDir, 'out', 'renderer', 'control-shield', 'index.html'), {
    query: { still: String(inContainer()) },
  });
}

/** The services the shield uses. */
type Deps = Pick<
  AppServices,
  'electron' | 'appDir' | 'control' | 'shell' | 'recorder' | 'shortcuts' | 'overlays' | 'tabs' | 'world'
>;

/** Where one element sits, in the shield's pixels, and the kind of element it is. */
export interface ShieldBox {
  /** The element's analysis id. */
  id: number;
  /** Its kind, which colours its outline. */
  type: string;
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

/** The element an action is aimed at. */
interface Aim {
  /** Its analysis id. */
  id: number;
  /** A selector that finds it, when the action named one. */
  selector?: string;
}

/** An analysis result, as the page reader returns it. */
export interface AnalysisResult {
  /** Whether the analysis ran. */
  ok?: boolean;
  /** What it found. */
  data?: AnalysisData;
}

/** How an isolated-world read is run: `retry: false` gives up on a page that is gone instead of waiting for it. */
interface EvalOptions {
  /** Whether to retry while the page's world is being rebuilt. */
  retry?: boolean;
}

/** One run of following the outlines; a newer run replaces it. */
interface TrackingRun {
  /** The next re-measure. */
  timer?: NodeJS.Timeout;
}

/**
 * Measures elements in the isolated world; it only reads. Each is found as the
 * analyzer finds it (shadow roots and iframes included), placed through any
 * same-origin frames, and mapped into the visual viewport so a pinch-zoom still
 * lines up. `__WANTED__` becomes the `[id, type, selector]` list.
 */
const MEASURE_JS = `(() => {
  const vv = window.visualViewport || { offsetLeft: 0, offsetTop: 0, scale: 1 };
  const find = (id, selector) =>
    window.__acFindElement?.(id) || window.__acQueryShadow?.(selector) || document.querySelector(selector);
  const place = (el) => {
    const r = el.getBoundingClientRect();
    let x = r.left, y = r.top;
    for (let f = el.ownerDocument.defaultView?.frameElement; f; f = f.ownerDocument.defaultView?.frameElement) {
      const fr = f.getBoundingClientRect();
      x += fr.left + f.clientLeft;
      y += fr.top + f.clientTop;
    }
    const k = vv.scale;
    return { x: (x - vv.offsetLeft) * k, y: (y - vv.offsetTop) * k, w: r.width * k, h: r.height * k };
  };
  // Painted, and on top somewhere: not faded out, hidden, covered by something else, or clipped away.
  const shows = (el) => {
    if (el.checkVisibility && !el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
    const r = el.getBoundingClientRect();
    const root = el.getRootNode();
    const lands = ([fx, fy]) => {
      const hit = root.elementFromPoint?.(r.left + r.width * fx, r.top + r.height * fy);
      return !!hit && (hit === el || el.contains(hit));
    };
    return [[0.5, 0.5], [0.2, 0.2], [0.8, 0.2], [0.2, 0.8], [0.8, 0.8]].some(lands);
  };
  return (__WANTED__).map(([id, type, selector]) => {
    const el = find(id, selector);
    if (!el || (__STRICT__ && !shows(el))) return null;
    const box = place(el);
    return box.w && box.h ? { id, type, ...box } : null;
  }).filter(Boolean);
})()`;

/**
 * Measures the visible elements an analysis found. `strict` also leaves out any the
 * page does not actually show (faded, covered or clipped): worth it once per analysis,
 * not on every re-measure while the outlines follow the page.
 */
function analysisBoxesJs(elements: readonly AnalyzedElement[], strict = false): string {
  const wanted = elements.filter((e) => e.visible).slice(0, MAX_ANALYSIS_BOXES);
  const list = JSON.stringify(wanted.map((e) => [e.id, e.type, e.selector]));
  return MEASURE_JS.replace('__WANTED__', () => list).replace('__STRICT__', String(strict));
}

/** Whether an agent holds the page: anything that re-analyzes it would renumber the agent's element ids. */
export function agentDriving(ctx: Pick<AppServices, 'control'>): boolean {
  return !ctx.control.snapshot().interactive;
}

/** The colour a target ring takes for each action, by the kind of element it acts on; anything else is a click. */
const ACT_TARGET_TYPES: Record<string, string> = { type: 'input', select: 'select' };

/** What a read of the page answers while an agent holds it. */
export const AGENT_HOLDS_PAGE = 'The agent is using this page. Take control to read it.';

/** The shield view and the popups it disables. */
export class ControlShield {
  /** The shield's BrowserView, created on first use. */
  view: BrowserView | null = null;
  /** Sign-in popups, enabled only while a person has control. */
  readonly popups = new Set<BrowserWindow>();
  /** The outlines being followed right now, or null; a newer run replaces it. */
  private tracking: TrackingRun | null = null;
  /** Analysis id to element name, from the last analysis, for the companion's lines. */
  private names: Map<number, string> = new Map();
  /** The main-process services. */
  private readonly deps: Deps;
  /** Input-triggered, deduplicated handoff confirmation. */
  readonly takeover: TakeoverPrompt;

  /** `deps` is the main-process context (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.takeover = new TakeoverPrompt(deps);
  }

  /** Throws unless a person holds control; every page-touching IPC call checks this. */
  requireHumanControl(): void {
    if (!this.deps.control.snapshot().interactive) throw new Error('Take control before interacting with this page');
  }

  /** A tab that takes focus while an agent drives hands it back to the shell. */
  keepFocusOnShell(): void {
    if (!this.deps.control.snapshot().interactive) this.deps.shell.window?.webContents.focus();
  }

  /** The control state changed: tell the shell, and fence the page accordingly. */
  controlChanged(state: ControlSnapshot): void {
    this.deps.shell.send('control-state', state);
    this.deps.recorder.controlLost(state);
    this.enableMenus(state.interactive);
    for (const popup of this.popups) if (!popup.isDestroyed()) popup.setEnabled(!drivenElsewhere(state));
    if (this.deps.shell.alive()) this.sync();
  }

  /** Greys out the page commands in the application menu. */
  enableMenus(interactive: boolean): void {
    for (const id of HUMAN_MENU_ITEMS) {
      const item = this.deps.electron.Menu.getApplicationMenu()?.getMenuItemById(id);
      if (item) item.enabled = interactive;
    }
  }

  /** A new sign-in popup: disabled only while someone else drives, forgotten once closed. */
  adoptPopup(childWindow: BrowserWindow): void {
    this.popups.add(childWindow);
    childWindow.setEnabled(!drivenElsewhere(this.deps.control.snapshot()));
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
    this.installShieldInput(view);
    return view;
  }

  /** Install shell shortcuts and input-triggered handoff on the shield alone. */
  private installShieldInput(view: BrowserView): void {
    this.deps.shortcuts.install(view.webContents);
    this.takeover.install(view.webContents);
  }

  /** Whether the page should be covered right now. */
  shouldCover(): boolean {
    const { shell, overlays, control } = this.deps;
    return shell.browsingMode && !overlays.names.size && !control.snapshot().interactive;
  }

  /**
   * Puts the shield over the active tab, or takes it away. The start page is the
   * shell's own, not a website, so there is nothing there to keep a person's hands
   * off, and its task box must take their typing: it is never covered.
   */
  sync(): void {
    const win: BrowserWindow | null = this.deps.shell.window;
    if (!win || win.isDestroyed()) return;
    const view = this.deps.tabs.getShownView() as BrowserView | null; // a tab shown in the window is a BrowserView; popups have windows of their own
    if (!this.shouldCover() || !view) return this.uncover(win);
    this.cover(win, view, this.prepare());
  }

  /** An agent began reading the page: the shield starts its scan, and stops following the last outlines. */
  analysisStarted(view: BrowserView): void {
    this.stopTracking();
    this.tell(view, { phase: 'scan' });
  }

  /** The analysis is back: outline what it found that the page really shows, then keep the outlines on their elements. A failed measurement outlines nothing. */
  async analysisFinished(view: BrowserView, raw: AnalysisResult | null | undefined): Promise<void> {
    this.stopTracking();
    if (!this.showing(view)) return;
    const elements = raw?.data?.elements || [];
    this.names = namesFrom(elements);
    const boxes = await this.outline(view, elements);
    const shown = new Set(boxes.map((b) => b.id));
    this.track(view, analysisBoxesJs(elements.filter((e) => shown.has(e.id))), Date.now() + SHIELD_TRACK_FOR_MS);
  }

  /** Measures what the page really shows of `elements`, and how dark the page is, and has the shield outline them; returns their boxes. */
  private async outline(view: BrowserView, elements: readonly AnalyzedElement[]): Promise<ShieldBox[]> {
    const read = (js: string): unknown => this.deps.world.evaluate(view, js, { retry: false });
    const [measured, tone] = await Promise.all([this.measure(view, analysisBoxesJs(elements, true)), pageTone(read)]);
    const boxes = measured || [];
    this.tell(view, { phase: 'found', boxes, ...(tone && { tone }) });
    return boxes;
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
    const id = idOf(params);
    const box = id === null ? null : await this.target(view, { id, selector: params?.selector }, action);
    this.tell(view, { phase: 'act', text, box, changes: changesPage(action) });
  }

  /** Where the element an action is aimed at sits now, coloured for the action, or null when it cannot be found. */
  private async target(view: BrowserView, aim: Aim, action: string): Promise<ShieldBox | null> {
    const type = Object.hasOwn(ACT_TARGET_TYPES, action) ? ACT_TARGET_TYPES[action] : 'button';
    const js = analysisBoxesJs([{ id: aim.id, type, selector: aim.selector, visible: true }]);
    return (await this.measure(view, js, { retry: false }))?.[0] || null;
  }

  /** Where the elements sit now, in the shield's pixels (the page may be zoomed, the shield is not); null when the page cannot be read. */
  private async measure(view: BrowserView, js: string, options?: EvalOptions): Promise<ShieldBox[] | null> {
    const boxes = await this.deps.world.evaluate<ShieldBox[] | null>(view, js, options).catch(() => null);
    if (!boxes || !this.view) return null;
    const k = view.webContents.getZoomFactor() / this.view.webContents.getZoomFactor();
    return boxes.map((b) => ({ ...b, x: b.x * k, y: b.y * k, w: b.w * k, h: b.h * k }));
  }

  /** Re-measures every SHIELD_TRACK_MS until `until`, so the outlines glide with the page as it scrolls or reflows. */
  private track(view: BrowserView, js: string, until: number): void {
    const run: TrackingRun = (this.tracking = {});
    run.timer = setTimeout(() => this.follow(run, view, js, until), SHIELD_TRACK_MS);
  }

  /** One re-measure of a tracking run; it tells the page, then schedules the next, unless the run was stopped or is over. A page that cannot be read has navigated away: its outlines fade and following stops. */
  private async follow(run: TrackingRun, view: BrowserView, js: string, until: number): Promise<void> {
    if (this.tracking !== run || !this.showing(view) || Date.now() > until) return;
    const boxes = await this.measure(view, js, { retry: false });
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
  }

  /** Whether the shield is over `view` right now. */
  private showing(view: BrowserView | null | undefined): boolean {
    return !!this.view && this.shouldCover() && this.deps.tabs.getActiveView() === view;
  }

  /** Hands the shield page one update, when it is over `view`. */
  private tell(view: BrowserView, update: object): void {
    if (!this.view || !this.showing(view)) return;
    this.view.webContents.executeJavaScript(`window.oyaShield?.(${JSON.stringify(update)})`).catch(() => {});
  }

  /** Takes the shield off the window. */
  private uncover(win: BrowserWindow): void {
    this.stopTracking();
    if (this.view) win.removeBrowserView(this.view);
  }

  /** Lays the shield exactly over `view`, on top, and keeps keyboard focus off the page. */
  private cover(win: BrowserWindow, view: BrowserView, shield: BrowserView): void {
    shield.setBounds(view.getBounds());
    if (!win.getBrowserViews().includes(shield)) win.addBrowserView(shield);
    win.setTopBrowserView(shield);
    if (view.webContents.isFocused()) win.webContents.focus();
  }
}
