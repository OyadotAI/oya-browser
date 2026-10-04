/**
 * The Oya start page: where a new tab opens. It shows while the active tab
 * is on the start page (the main process reports it in the tab list), and a
 * task typed there goes to the agent through Ask, which already handles
 * sign-in, the model key and every answer, so the start page never
 * duplicates any of it.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ShellViewModel } from '../../../app/shell-view-model.ts';

/** Where the start page hands a task: Ask (wired by the root to the Ask feature's ViewModel). */
export interface StartAsker {
  /** Opens the panel on Ask and sends `text`. */
  ask(text: string): unknown;
  /** Ask is already waiting on an answer, so a new task would be dropped. */
  busy(): boolean;
}

/** What the start page shows. */
export interface StartState {
  /** The active tab is on the start page. */
  home: boolean;
  /** Counts arrivals on the start page; the view replays the arrival and focuses the task box on each. */
  arrivals: number;
}

/** What the start page uses. */
export interface StartDeps {
  /** The tab list. */
  bridge: Pick<OyaBrowser, 'onTabsUpdated'>;
  /** Told whether the start page is up (body.on-home follows it). */
  shell: Pick<ShellViewModel, 'setOnHome'>;
  /** Where tasks go. */
  asker: StartAsker;
}

/** The start page. */
export class StartViewModel extends ViewModel<StartState> {
  /** Where tasks go. */
  private readonly asker: StartAsker;
  /** Told whether the start page is up. */
  private readonly shell: Pick<ShellViewModel, 'setOnHome'>;

  /** Hidden until a tab list says the active tab is home. */
  constructor(deps: StartDeps) {
    super({ home: false, arrivals: 0 });
    this.asker = deps.asker;
    this.shell = deps.shell;
    this.own(deps.bridge.onTabsUpdated((tabs) => this.follow(!!tabs.find((tab) => tab.active)?.home)));
  }

  /** Hands a task to the agent; answers whether it went (so the box clears), not for an empty one or while Ask is busy. */
  ask(text: string): boolean {
    const task = text.trim();
    if (!task || this.asker.busy()) return false;
    void this.asker.ask(task);
    return true;
  }

  /** The start page shows exactly while the active tab is on it; each arrival plays again. */
  private follow(home: boolean): void {
    if (home === this.state.home) return;
    this.shell.setOnHome(home);
    this.set(home ? { home, arrivals: this.state.arrivals + 1 } : { home });
  }
}
