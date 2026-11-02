/**
 * Which tabs agents drive. While the server's agents hold control, only their own
 * tabs are fenced: each agent (a chat run, an MCP session, a playbook run; its
 * "lane", carried on every command) drives one tab of its own, and the person can
 * switch to, open and use every other tab at the same time. Every agent command
 * keeps acting on its lane's tab whichever one the window shows.
 *
 * A connected desktop sits in agent mode even when nothing runs, so no tab is
 * fenced until an agent starts on one (its first page command, or a chat sent from
 * it); a run's end lets it go. Local automation (a CDP client, a workflow check) and
 * another operator's hold still take the whole browser, as before.
 */
import { AsyncLocalStorage } from 'node:async_hooks';
import type { AppServices } from '../app/services.ts';
import type { TabView } from '../tabs/types.ts';
import type { ControlSnapshot } from './control-state.ts';
import { LANE_IDLE_MS } from './constants.ts';
import { HOME_URL } from '../tabs/constants.ts';

/** What the claims read: who drives, and the tabs. */
type Deps = Pick<AppServices, 'control' | 'tabs' | 'windows'>;

/** The lane of a command that names none: one agent, as before lanes. */
export const DEFAULT_LANE = 'default';

/** The lane the running command belongs to. */
const lanes = new AsyncLocalStorage<string>();

/** Runs `work` as lane `lane`'s, so every agent-tab read inside it means that lane's tab. */
export function inLane<T>(lane: unknown, work: () => T): T {
  return lanes.run(typeof lane === 'string' && lane ? lane : DEFAULT_LANE, work);
}

/** The running command's lane. */
const currentLane = (): string => lanes.getStore() ?? DEFAULT_LANE;

/** The agents' tabs while control is scoped to them. */
export class AgentTab {
  /** Each lane's tab. */
  private readonly claims = new Map<string, number>();
  /** Each lane's first tab (the one its chat was sent from, or the shown one it started on): the one tab it did not open that it may work in. */
  private readonly homes = new Map<string, number>();
  /** The lane that opened each tab an agent opened: an agent may close only the tabs it opened itself. */
  private readonly openers = new Map<number, string>();
  /** Each lane's lapse: a lane quiet for LANE_IDLE_MS has finished. */
  private readonly lapses = new Map<string, ReturnType<typeof setTimeout>>();
  /** Control and the tabs. */
  private readonly deps: Deps;
  /** Hears a claim move, so every window's shield, toolbar and strip follow it. */
  private readonly changed: () => void;

  /** `deps` is the main-process services; `changed` hears a claim move. */
  constructor(deps: Deps, changed: () => void = () => {}) {
    this.deps = deps;
    this.changed = changed;
  }

  /** The running lane's tab, or null when it has none. */
  get id(): number | null {
    return this.claims.get(currentLane()) ?? null;
  }

  /** Every tab an agent drives. */
  claimed(): number[] {
    return [...this.claims.values()];
  }

  /** Whether control is scoped to agents' tabs: the server's agents drive, with no local automation attached. */
  scoped(state: ControlSnapshot = this.deps.control.snapshot()): boolean {
    return !!state.connected && state.mode === 'agent' && !state.localClients && !state.interactive;
  }

  /** On every control change: anything but the server's agents driving ends every claim. */
  sync(state: ControlSnapshot): void {
    if (!this.scoped(state) && this.claims.size) this.update(() => (this.claims.clear(), this.homes.clear()));
  }

  /**
   * An agent starts on a page with no tab of its own yet: alone, it takes the shown tab,
   * as one agent always has; beside other agents, it gets a fresh background tab on the start page, so it
   * never lands in another agent's tab or the one the person is using.
   */
  start(): void {
    if (!this.scoped() || this.claims.has(currentLane())) return;
    const { tabs } = this.deps;
    if (this.claims.size) this.opened(tabs.createTab(HOME_URL, false));
    else this.claim(tabs.activeTabId);
  }

  /**
   * The running lane's run ended: its tab is the person's again, the tabs it opened
   * close, and a person watching the agent is brought back to the tab it started in,
   * so they never hunt for it among the agent's leftovers. Tabs the person opened stay.
   */
  release(): void {
    const lane = currentLane();
    const home = this.homes.get(lane);
    const watching = this.watching(lane);
    if (this.claims.has(lane)) this.update(() => this.forget(lane));
    this.closeOpened(lane);
    if (watching && home !== undefined && this.deps.tabs.find(home)) this.deps.tabs.activateTab(home);
  }

  /** Whether the window shows lane `lane`'s tab or one it opened. */
  private watching(lane: string): boolean {
    const shown = this.deps.tabs.activeTabId;
    return shown !== null && (shown === this.claims.get(lane) || this.openers.get(shown) === lane);
  }

  /** Closes every tab lane `lane` opened, but the one it started in, and forgets them. */
  private closeOpened(lane: string): void {
    const home = this.homes.get(lane);
    for (const [id, opener] of [...this.openers]) {
      if (opener !== lane) continue;
      this.openers.delete(id);
      if (id !== home) this.deps.tabs.closeTab(id);
    }
  }

  /** The running lane now drives tab `id` (a tab it opened, one it switched to, the one a chat was sent from). */
  claim(id: number | null | undefined): void {
    if (!this.scoped() || id === null || id === undefined || this.claims.get(currentLane()) === id) return;
    if (!this.homes.has(currentLane())) this.homes.set(currentLane(), id);
    this.update(() => this.claims.set(currentLane(), id));
    this.keepAlive(currentLane());
  }

  /** A tab the running lane opened becomes its tab, and one it may close; answers its id. */
  opened(id: number): number {
    this.openers.set(id, currentLane());
    this.claim(id);
    return id;
  }

  /** Whether the running lane may work in tab `id`: its first tab or one it opened, never the person's other tabs or another agent's. */
  mayWorkIn(id: number | null | undefined): boolean {
    return this.openedByThisLane(id) || (id !== null && id !== undefined && this.homes.get(currentLane()) === id);
  }

  /** Whether the running lane opened tab `id`; a person's tab, or another agent's, is never its to close. */
  openedByThisLane(id: number | null | undefined): boolean {
    return id !== null && id !== undefined && this.openers.get(id) === currentLane();
  }

  /** The page a command acts on, taking a tab first when its lane has none yet; the lane is alive again. */
  drive(): TabView | null {
    this.start();
    this.keepAlive(currentLane());
    return this.view();
  }

  /** Restarts lane `lane`'s lapse. */
  private keepAlive(lane: string): void {
    clearTimeout(this.lapses.get(lane));
    if (!this.claims.has(lane)) return void this.lapses.delete(lane);
    const lapse = setTimeout(() => this.claims.has(lane) && this.update(() => this.forget(lane)), LANE_IDLE_MS);
    this.lapses.set(lane, (lapse.unref?.(), lapse));
  }

  /** The running lane's page: its own tab while scoped (null once that tab is gone, never another's), else the active one. */
  view(): TabView | null {
    const id = this.id;
    if (!this.scoped() || id === null) return this.deps.tabs.getActiveView();
    return (this.deps.tabs.find(id)?.view as TabView | undefined) ?? null;
  }

  /** The running lane's tab id, as list_tabs reports it active. */
  tabId(): number | null {
    return this.scoped() && this.id !== null ? this.id : (this.deps.tabs.activeTabId ?? null);
  }

  /** Whether the person may use tab `tabId` right now; null asks about something new, such as a new tab. Incognito tabs are always the person's. */
  humanMay(tabId: number | null | undefined): boolean {
    const state = this.deps.control.snapshot();
    if (state.interactive || this.incognito(tabId)) return true;
    return this.scoped(state) && (tabId === null || !this.claimed().includes(tabId!));
  }

  /** Whether `tabId` is in an incognito window, where no agent ever acts. */
  private incognito(tabId: number | null | undefined): boolean {
    return tabId != null && !!this.deps.windows?.owner(tabId)?.tabs.privateSession;
  }

  /** Whether another lane drives tab `tabId`. */
  othersDrive(tabId: number | null | undefined): boolean {
    return [...this.claims].some(([lane, id]) => id === tabId && lane !== currentLane());
  }

  /** Throws unless the person may use tab `tabId` (null: something new, such as a new tab). */
  requireFor(tabId: number | null | undefined): void {
    if (!this.humanMay(tabId)) throw new Error('Switch to Human Mode before using this page');
  }

  /** A link the agent's tab `from` opened in tab `to`: whichever lane drives `from` drives `to` now, and opened it, as with open_tab. */
  handOff(from: number, to: number): void {
    const lane = [...this.claims].find(([, id]) => id === from)?.[0];
    if (lane === undefined) return;
    this.openers.set(to, lane);
    this.update(() => this.claims.set(lane, to));
    this.keepAlive(lane);
  }

  /** Whether the window shows the running lane's tab, so what it opens is shown too. */
  following(): boolean {
    return !this.scoped() || this.id === null || this.deps.tabs.activeTabId === this.id;
  }

  /** Lane `lane` has finished: its tab and its first tab are the person's again. */
  private forget(lane: string): void {
    this.claims.delete(lane);
    this.homes.delete(lane);
    clearTimeout(this.lapses.get(lane));
    this.lapses.delete(lane);
  }

  /** Changes the claims, then tells the windows. */
  private update(change: () => unknown): void {
    change();
    this.changed();
  }
}
