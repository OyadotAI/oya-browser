/**
 * A tab whose page stops responding, and a record of it. A hung renderer goes
 * blank when its tab is shown again and stops its audio, and nothing said so: the
 * tab now says it is not responding, and a log in the user-data folder keeps
 * when each hang, recovery and renderer or helper-process death was detected, so
 * the cause (a peer tab, the agent's tab, the GPU or audio process) can be found.
 * A person's press on the control shield is kept too, with who held control and
 * what was mounted over the page, so "clicks do nothing" can be told apart from a hang.
 */
import type { Details } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { ControlSnapshot } from '../control/control-state.ts';
import type { JsonFile } from '../app/json-file.ts';
import { privateSession } from '../native-contexts/index.ts';
import { TAB_HEALTH_ENTRIES } from './constants.ts';
import type { Tab } from './types.ts';

/** What happened: to a tab's page (`probe-failed`: it did not answer a person's Reload), to one of the app's helper processes (`process-gone`), or a person pressing on the control shield (`shield-press`). */
export type TabHealthEvent = 'probe-failed' | 'gone' | 'process-gone' | 'shield-press';

/** The control fields a shield press keeps: enough to tell a lapsed hold, a handoff and an agent's claim apart. */
const PRESS_CONTROL = [
  'mode',
  'mine',
  'revision',
  'expiresAt',
  'connected',
  'busy',
  'busyAction',
  'interactive',
  'reclaiming',
] as const;

/** Who held control when the shield was pressed. */
export type PressControl = Partial<Pick<ControlSnapshot, (typeof PRESS_CONTROL)[number]>>;

/** One view mounted in the window, as a shield press records it. */
export interface StackEntry {
  /** The shield, one of the window's tabs, or anything else mounted there. */
  kind: 'shield' | 'tab' | 'other';
  /** The tab's id. */
  tab?: number;
  /** The tab page's site only. */
  origin?: string;
}

/** A person's press on the shield, and the window it landed in. */
export interface ShieldPress {
  /** The window's own tabs: each window keeps its own list. */
  tabs: readonly Tab[];
  /** The window's mounted views, as Electron lists them (bottom first). */
  views: readonly unknown[];
  /** The shield's own view. */
  shield: unknown;
  /** The control state at the press. */
  control: ControlSnapshot;
  /** The tab the window shows. */
  tab: number | null;
}

/** One line of the log. Tab fields are absent for a helper process, process fields for a tab. */
export interface TabHealthEntry {
  /** When the app noticed, not when it began: a background hang shows only once something waits on the page. */
  detectedAt: string;
  /** What happened. */
  event: TabHealthEvent;
  /** The tab's id. */
  tab?: number;
  /** The page's site only: a full address can carry tokens, and a private tab's is never written. */
  origin?: string;
  /** The renderer's OS process id (0 once it is gone). */
  pid?: number;
  /** Whether an agent held the tab. */
  agent?: boolean;
  /** Other tabs in the same renderer process: a hang in one is a hang in all of them. */
  peers?: number[];
  /** Why a renderer or helper process went away. */
  reason?: string;
  /** Which helper process went away (GPU, Audio Service, …). */
  process?: string;
  /** For a shield press: who held control. */
  control?: PressControl;
  /** For a shield press: the window's mounted views, top first; private tabs are left out. */
  views?: StackEntry[];
}

/** What the log reads: every tab, and which ones an agent holds. */
type LogDeps = Pick<AppServices, 'tabs' | 'agentTab'>;

/** The site of the tab's page, or '' for a private tab or an address with none. */
function siteForLog(tab: Tab): string {
  if (privateSession(tab.view.webContents.session)) return '';
  try {
    return new URL(tab.view.webContents.getURL()).origin;
  } catch {
    return '';
  }
}

/** The control fields a press keeps. */
function pressControl(control: ControlSnapshot): PressControl {
  return Object.fromEntries(PRESS_CONTROL.filter((key) => key in control).map((key) => [key, control[key]]));
}

/** How the press records `view`: null for a private tab, which is left out entirely. */
function stackEntry(view: unknown, press: ShieldPress): StackEntry | null {
  if (view === press.shield) return { kind: 'shield' };
  const tab = press.tabs.find((t) => t.view === view);
  if (!tab) return { kind: 'other' };
  if (privateSession(tab.view.webContents.session)) return null;
  return { kind: 'tab', tab: tab.id, origin: siteForLog(tab) };
}

/** The window's views, top first, without private tabs. */
function stackOf(press: ShieldPress): StackEntry[] {
  return [...press.views].reverse().flatMap((view) => stackEntry(view, press) ?? []);
}

/** Keeps the newest TAB_HEALTH_ENTRIES health events in one owner-only file. */
export class TabHealthLog {
  /** The file. */
  private readonly file: JsonFile<TabHealthEntry[]>;
  /** The tabs and the agent's claims. */
  private readonly deps: LogDeps;

  /** `file` is the log in the user-data folder; `deps` are the main-process services. */
  constructor(deps: LogDeps, file: JsonFile<TabHealthEntry[]>) {
    this.deps = deps;
    this.file = file;
  }

  /** Records what happened to `tab`'s page, with who else shares its renderer. */
  record(tab: Tab, event: TabHealthEvent, reason?: string): void {
    const pid = tab.view.webContents.getOSProcessId?.() || 0;
    const peers = pid ? this.peersOf(tab, pid) : [];
    const agent = this.deps.agentTab.claimed().includes(tab.id);
    this.append({ event, tab: tab.id, origin: siteForLog(tab), pid, agent, peers, ...(reason ? { reason } : {}) });
  }

  /** Records a helper process (GPU, audio, a utility) that went away: no tab event says so. */
  processGone(details: Details): void {
    this.append({ event: 'process-gone', process: details.name || details.type, reason: details.reason });
  }

  /** Records a person's press on the control shield: who held control, and what was mounted over the page. */
  shieldPress(press: ShieldPress): void {
    const control = pressControl(press.control);
    const views = stackOf(press);
    const shown = press.tabs.find((t) => t.id === press.tab);
    if (!shown || privateSession(shown.view.webContents.session))
      return this.append({ event: 'shield-press', control, views });
    const agent = this.deps.agentTab.claimed().includes(shown.id);
    this.append({ event: 'shield-press', tab: shown.id, agent, control, views });
  }

  /** The other tabs whose page runs in process `pid`. */
  private peersOf(tab: Tab, pid: number): number[] {
    const others = this.deps.tabs.list.filter((t) => t !== tab && !t.window && !t.view.webContents.isDestroyed());
    return others.filter((t) => t.view.webContents.getOSProcessId?.() === pid).map((t) => t.id);
  }

  /** Adds one entry, keeping the newest; a missing or corrupt file starts over. */
  private append(entry: Omit<TabHealthEntry, 'detectedAt'>): void {
    const saved = this.file.read();
    const entries = Array.isArray(saved) ? saved : [];
    entries.push({ detectedAt: new Date().toISOString(), ...entry });
    this.file.write(entries.slice(-TAB_HEALTH_ENTRIES));
  }
}
