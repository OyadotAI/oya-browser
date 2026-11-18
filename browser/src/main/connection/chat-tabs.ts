/**
 * Keeps each chat in flight in its tab across a dropped socket. Losing the socket ends every
 * agent's claim (control is no longer the agents'), and a run that carries on after the reconnect
 * would otherwise start over in the shown tab, or a new one, instead of the tab it was working in.
 * So where each chat's agent is gets noted before a control change is applied, and given back after.
 */
import { inLane } from '../control/agent-tab.ts';
import type { AppServices } from '../app/services.ts';
import { tabLane } from './chat.ts';

/** The chats in flight, the agents' tabs, and the open tabs. */
type Deps = Pick<AppServices, 'chatRuns' | 'agentTab' | 'tabs'>;

/** Where each chat's agent was, by run, so one run's tab is never given to the next run in the same tab. */
export class ChatTabs {
  /** The main-process services. */
  private readonly deps: Deps;
  /** The tab each run in flight last drove. */
  private readonly last = new WeakMap<AbortController, number>();

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** Notes the tab each chat's agent drives now; called before a control change ends the claims. */
  note(): void {
    for (const [tab, run] of this.deps.chatRuns) {
      const driven = inLane(tabLane(tab), () => this.deps.agentTab.id);
      if (driven !== null) this.last.set(run, driven);
    }
  }

  /**
   * Gives each chat whose agent has no tab the one it drove, once the agents drive again. Safe on
   * every control change: an agent that has a tab keeps it, and none is given while a person or
   * local automation holds control.
   */
  restore(): void {
    for (const [tab, run] of this.deps.chatRuns) inLane(tabLane(tab), () => this.giveBack(tab, this.last.get(run)));
  }

  /** The running lane goes back to tab `was`, or to the tab its chat was sent from when that one has closed. */
  private giveBack(home: number, was: number | undefined): void {
    const { agentTab, tabs } = this.deps;
    if (agentTab.id !== null) return;
    agentTab.claim(home);
    if (was !== undefined && was !== home && tabs.find(was)) agentTab.claim(was);
  }
}
