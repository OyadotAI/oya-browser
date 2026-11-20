/**
 * One agent panel per tab: each tab keeps its own conversation and its own run, so an
 * agent working in one tab never fills the panel of another, and the person can start
 * another agent in a new tab while the first keeps going. The panel shows the
 * conversation of the tab its window shows; a closed tab's conversation goes with it.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { TabSummary } from '../../../../shared/ipc.ts';
import { AskViewModel, type AskServices } from './ask-view-model.ts';
import { readBase64, type ReadFile } from './files-view-model.ts';

/** What the panel shows: the shown tab's conversation. */
export interface AskSessionsState {
  /** The shown tab's conversation. */
  current: AskViewModel;
}

/** Every tab's conversation, and the one the panel shows. */
export class AskSessions extends ViewModel<AskSessionsState> {
  /** Each tab's conversation, by tab id. */
  private readonly sessions = new Map<number, AskViewModel>();
  /** Pre-tab conversations finishing their last run before disposal. */
  private readonly retiring = new Set<AskViewModel>();
  /** The main process and the panel. */
  private readonly services: AskServices;
  /** Reads attachments. */
  private readonly readFile: ReadFile;

  /** Starts on a conversation with no tab yet, follows the tab strip, and is the Ask pane's Clear. */
  constructor(services: AskServices, readFile: ReadFile = readBase64) {
    super({ current: new AskViewModel(services, readFile) });
    this.services = services;
    this.readFile = readFile;
    this.own(services.bridge.onTabsUpdated((tabs) => this.follow(tabs)));
    this.own(services.panel.onClear('chat', () => this.state.current.clear()));
    this.own(() => this.disposeAll());
  }

  /** Hands the shown tab's agent a task (the start page's way in). */
  ask(text: string): Promise<void> {
    return this.state.current.ask(text);
  }

  /** Whether any tab's agent is working. */
  anyBusy(): boolean {
    const busy = (list: Iterable<AskViewModel>) => [...list].some((s) => s.state.sending);
    return this.state.current.state.sending || busy(this.sessions.values()) || busy(this.retiring);
  }

  /** Whether the shown tab's agent is busy. */
  busy(): boolean {
    return this.state.current.state.sending;
  }

  /** The strip changed: show the active tab's conversation, and let closed tabs' conversations go. */
  private follow(tabs: TabSummary[]): void {
    this.forgetClosed(new Set(tabs.map((tab) => tab.id)));
    const active = tabs.find((tab) => tab.active)?.id;
    if (active === undefined) return this.resetEmpty();
    if (!this.sessions.has(active)) this.sessions.set(active, new AskViewModel(this.services, this.readFile, active));
    this.show(this.sessions.get(active)!);
  }

  /** Resets to an idle pre-tab conversation when the last tab closes (e.g. sign-out). */
  private resetEmpty(): void {
    if (this.state.current.tabId === null && !this.state.current.state.sending) return;
    this.show(new AskViewModel(this.services, this.readFile));
  }

  /** Shows `next`; any replaced pre-tab conversation disposes now or once its run finishes. */
  private show(next: AskViewModel): void {
    const previous = this.state.current;
    if (previous === next) return;
    this.set({ current: next });
    if (previous.tabId === null) this.retire(previous);
  }

  /** Disposes a replaced pre-tab conversation now, or as soon as its in-flight run ends. */
  private retire(session: AskViewModel): void {
    if (!session.state.sending) return session.dispose();
    this.retiring.add(session);
    const off = session.subscribe(() => {
      if (session.state.sending) return;
      off();
      this.retiring.delete(session);
      session.dispose();
    });
  }

  /** Disposes the conversations of tabs that closed, stopping any run still going there. */
  private forgetClosed(open: Set<number>): void {
    for (const [tab, session] of this.sessions) {
      if (open.has(tab)) continue;
      if (session.state.sending) session.stop();
      session.dispose();
      this.sessions.delete(tab);
    }
  }

  /** Disposes every conversation. */
  private disposeAll(): void {
    for (const session of [...this.sessions.values(), ...this.retiring]) session.dispose();
    this.retiring.clear();
    if (this.state.current.tabId === null) this.state.current.dispose();
  }
}
