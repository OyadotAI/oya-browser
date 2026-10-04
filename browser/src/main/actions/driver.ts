/**
 * PageDriver: what every page command shares. It holds the composition root's (src/main/main.ts) tab state
 * and helpers, the keyboard and mouse, waits for loads, finds elements, and
 * dispatches server and dev panel commands through their command maps.
 */
import type { WebContents } from 'electron';
import type { PageView } from '../cdp/cdp.ts';
import type { Keyboard } from '../input/keyboard.ts';
import type { Mouse } from '../input/mouse.ts';
import { sleep } from '../input/timing.ts';
import { findElementJs, actionScript } from './scripts.ts';
import { renderedAnalysis, type FormatSettings } from './page-format.ts';
import { TAB_READY_TIMEOUT_MS, LOAD_TIMEOUT_MS, EMPTY_ANALYSIS_RETRY_MS } from './constants.ts';
import { PAGE_COMMANDS } from './page-commands.ts';
import { DEV_COMMANDS, UNGUARDED_DEV_COMMANDS } from './dev-commands.ts';
import type { AnalysisData, CommandId, CommandParams, Located, ScriptResult } from './types.ts';

/** A tab as far as waiting for it goes: its view, and its first load while that runs. */
export interface TabView {
  /** The view that shows the tab. */
  view: PageView;
  /** Settles once CDP setup and the first navigation finish. */
  ready?: Promise<unknown>;
}

/** A tab as the tabs list it. */
export interface DriverTab extends TabView {
  /** The tab's number. */
  id: number;
  /** Its page title. */
  title: string;
  /** Its page address. */
  url: string;
}

/** the composition root's (src/main/main.ts) tab state (read through getters) and the helpers that act on it, as the composition root hands them over. */
export interface PageDriverDeps extends FormatSettings {
  /** Goes to an address the way the address bar does: search, cookies and recording included. */
  navigate(url: string): Promise<unknown>;
  /** The view of the active tab, or null when there is none. */
  getActiveView(): PageView | null;
  /** Pulls the cookie pool's cookies for an address before loading it. */
  pullCookiesFor(url: string): Promise<unknown>;
  /** Loads the analyzer into the page, if it is not there yet. */
  injectScripts(view: PageView): Promise<unknown>;
  /** Evaluates an expression in the analyzer's isolated world and answers its value. */
  worldEval<T = unknown>(view: PageView, expression: string): Promise<T>;
  /** Answers a server command: success, its data, the error and the error's code. */
  sendResult(id: CommandId, ok: boolean, data?: unknown, error?: string, code?: string): void;
  /** Opens a tab on `url`, active when `activate`; answers its number. */
  createTab(url: string, activate: boolean): number;
  /** Closes a tab by number. */
  closeTab(id: number): void;
  /** Throws unless the person has taken control of the page. */
  requireHumanControl(): void;
  /** Shows on the control shield that an analysis has started. */
  analysisStarted(view: PageView): void;
  /** Shows the finished analysis on the control shield. */
  analysisFinished(view: PageView, raw: unknown): void;
  /** Says on the shield what an action is doing; never waited for. */
  narrate?(view: PageView, action: string, params: CommandParams | undefined): unknown;
  /** The open tabs. */
  tabs(): DriverTab[];
  /** The active tab's number, or null when there is none. */
  activeTabId(): number | null;
}

/** Everything a PageDriver is built from: the composition root's (src/main/main.ts) helpers, and the human-like input devices. */
export interface PageDriverInput extends PageDriverDeps {
  /** Types and presses keys like a person. */
  keyboard: Keyboard;
  /** Moves and clicks the pointer like a person. */
  mouse: Mouse;
}

/** A dev panel answer, given by return value. */
export type DevAnswer = ScriptResult | null;

/** The events that end a load, successful or not. */
const LOAD_END_EVENTS = ['did-finish-load', 'did-fail-load'] as const;

/** What a caller is told about an action this browser has no handler or script for. A newer server may know it. */
const unknownAction = (action: string): string =>
  `This Oya Browser does not know the action "${action}". Update the app, then send it again.`;

/** Calls `resolve` once `contents` ends a load or `ms` pass, removing its listeners either way. */
function untilLoaded(contents: WebContents, ms: number, resolve: () => void): void {
  const events: NodeJS.EventEmitter = contents;
  const done = (): void => {
    clearTimeout(timer);
    if (!contents.isDestroyed()) for (const event of LOAD_END_EVENTS) events.off(event, done);
    resolve();
  };
  const timer = setTimeout(done, ms);
  for (const event of LOAD_END_EVENTS) events.once(event, done);
}

/** Whether an analysis found at least one element. */
const foundElements = (raw: ScriptResult<AnalysisData> | null): boolean => !!raw?.data?.elements?.length;

/** Drives the active page for the command socket and the dev panel. */
export class PageDriver {
  /** The tab state and helpers from the composition root (src/main/main.ts). */
  readonly deps: PageDriverDeps;
  /** The keyboard every command types with. */
  readonly keyboard: Keyboard;
  /** The pointer every command moves, which remembers where it was left. */
  readonly mouse: Mouse;

  /** `deps` is the composition root's (src/main/main.ts) tab state and helpers, with the keyboard and mouse to drive the page by. */
  constructor(deps: PageDriverInput) {
    this.deps = deps;
    this.keyboard = deps.keyboard;
    this.mouse = deps.mouse;
  }

  /** The active tab's view; a command that needs one throws without it. */
  activeView(): PageView {
    const view = this.deps.getActiveView();
    if (!view) throw new Error('No active tab');
    return view;
  }

  /**
   * Wait for a tab's first load before driving it, but never unconditionally.
   *
   * tab.ready only settles once CDP setup and the initial navigation finish. A
   * page that never finishes doing either used to block every later command on
   * that tab with no result and no error, so the caller just timed out. That is
   * how a browser image whose renderers would not start looked like a dead
   * server rather than a broken page.
   */
  waitForTabReady(tab: Partial<TabView> | null | undefined): Promise<unknown> {
    if (!tab?.ready) return Promise.resolve();
    // A rejected first load is not this command's problem: it is about to
    // navigate somewhere else anyway.
    return Promise.race([tab.ready.catch(() => {}), sleep(TAB_READY_TIMEOUT_MS)]);
  }

  /**
   * Settle when the page finishes or fails loading, or give up.
   *
   * Every listener is removed on the way out. The old version attached one per
   * call and dropped it only when the load fired, so an agent session driving a
   * page that never finishes piled them onto the same webContents.
   */
  waitForLoad(view = this.deps.getActiveView(), ms = LOAD_TIMEOUT_MS): Promise<void> {
    if (!view || view.webContents.isDestroyed()) return Promise.resolve();
    return new Promise((resolve) => untilLoaded(view.webContents, ms, resolve));
  }

  /** Loads the analyzer, then finds `selector`: `{ ok, data: { x, y, tag, editable, inIframe } }` or `{ ok: false, error }`. */
  async locate(view: PageView, selector: string): Promise<Located | null> {
    await this.deps.injectScripts(view);
    return this.deps.worldEval<Located | null>(view, findElementJs(selector));
  }

  /** Like locate(), but a missing element is answered to the caller here; resolves to the element or null. */
  async find(id: CommandId, view: PageView, selector: string): Promise<Located | null> {
    const info = await this.locate(view, selector);
    if (info?.ok) return info;
    this.deps.sendResult(id, false, null, info?.error || 'Element not found');
    return null;
  }

  /** Everything after tab management in runCommand; answers through sendResult. */
  async runPageAction(id: CommandId, action: string, params: CommandParams | undefined, view: PageView): Promise<void> {
    if (!Object.hasOwn(PAGE_COMMANDS, action)) return this.runInjected(id, action, params, view);
    // Said on the shield while the action runs; the agent never waits for it.
    this.deps.narrate?.(view, action, params);
    await PAGE_COMMANDS[action](this, id, params, view);
  }

  /**
   * An action with no handler of its own runs as an analyzer script; an analysis
   * is shown on the control shield. One this browser does not know is refused
   * here: its name is the caller's text and is never evaluated in the page.
   */
  async runInjected(id: CommandId, action: string, params: CommandParams | undefined, view: PageView): Promise<void> {
    if (actionScript(action, params) === null)
      return this.deps.sendResult(id, false, null, unknownAction(action), 'action_unsupported');
    await this.deps.injectScripts(view);
    if (action === 'analyze') this.deps.analysisStarted(view);
    const raw = await this.analysed(action, params, view);
    if (action === 'analyze') this.deps.analysisFinished(view, raw);
    const result = action === 'analyze' ? renderedAnalysis(this.deps, raw, params) : raw;
    this.deps.sendResult(id, result?.ok ?? true, result?.data, result?.error);
  }

  /**
   * The script's result. An analysis that found no element at all is read once
   * more after a pause: an app that answered before it drew anything is still
   * loading, and an empty page would tell the agent the site is broken.
   */
  async analysed(action: string, params: CommandParams | undefined, view: PageView): Promise<ScriptResult | null> {
    const script = (): string => actionScript(action, params) ?? '';
    const raw = await this.deps.worldEval<ScriptResult<AnalysisData> | null>(view, script());
    if (action !== 'analyze' || foundElements(raw)) return raw;
    await new Promise((resolve) => setTimeout(resolve, EMPTY_ANALYSIS_RETRY_MS));
    return this.deps.worldEval<ScriptResult | null>(view, script());
  }

  /** The dev panel's quick actions; answers by return value, never throws. */
  async runDevAction(action: string, params?: CommandParams): Promise<DevAnswer> {
    const view = this.deps.getActiveView();
    if (!view && action !== 'list-tabs') return { ok: false, error: 'No active tab' };
    try {
      // Only list-tabs runs without a view, and it never reads one.
      return await this.devCommand(action, view as PageView, params);
    } catch (err) {
      return { ok: false, error: (err as Error).message };
    }
  }

  /** Runs one dev action; everything but a read needs human control. */
  devCommand(action: string, view: PageView, params: CommandParams | undefined): DevAnswer | Promise<DevAnswer> {
    if (!UNGUARDED_DEV_COMMANDS.includes(action)) this.deps.requireHumanControl();
    if (!Object.hasOwn(DEV_COMMANDS, action)) return { ok: false, error: 'Unknown action: ' + action };
    return DEV_COMMANDS[action](this, view, params);
  }
}
