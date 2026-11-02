/**
 * Ask: the conversation sent to this browser's agent on the server, one run per
 * tab at a time, with control lent to the agent while it runs. Each tab's chat is
 * its own agent (lane `tab-<id>`), driving that tab beside the other tabs' agents.
 * The panel and the routines both ask through it.
 */
import { inLane } from '../control/agent-tab.ts';
import type { AppServices } from '../app/services.ts';
import { ServerApi } from './server-api.ts';
import { ERROR_PREVIEW_CHARS } from './constants.ts';

/** The services a chat uses. */
type Deps = Pick<
  AppServices,
  'config' | 'socket' | 'control' | 'shield' | 'chatAbort' | 'chatRuns' | 'agentTab' | 'tabs' | 'windows'
>;

/** The lane of tab `tab`'s chat: its agent drives that tab, beside other tabs' agents. */
export const tabLane = (tab: number): string => `tab-${tab}`;

/** The tab a lane's events belong to, or null for a lane no tab owns. */
export function tabOfLane(lane: unknown): number | null {
  const match = /^tab-(\d+)$/.exec(String(lane ?? ''));
  return match ? Number(match[1]) : null;
}

/** What the agent answers: its reply, or `{ error }`. */
export interface ChatAnswer {
  /** Why it could not answer, or STOPPED. */
  error?: string;
  /** Its reply. */
  text?: string;
  /** Whatever else the server answered. */
  [key: string]: unknown;
}

/** What a chat the person stopped answers. */
export const STOPPED = 'Stopped';
/** What a chat answers while the agent is already on another task (a routine, or another chat). */
const BUSY = 'The agent is busy with another task. Stop it first, or try again when it finishes.';
/** What a chat answers when the server hung up mid-run (fetch says "terminated"), as a restart does. */
const CUT_OFF =
  'The connection to the server dropped mid-run, likely a server restart. Check where the page got to, then ask again.';
/** What a chat answers without a signed-in socket. */
const OFFLINE = 'Not connected to server';
/** What a chat answers from an incognito tab: agents never see or drive incognito pages. */
const INCOGNITO = "Oya's agents can't use incognito windows";

/** The server's JSON answer, or an error quoting what it sent instead. */
async function readChatAnswer(res: Response): Promise<ChatAnswer> {
  const text = await res.text();
  try {
    return JSON.parse(text);
  } catch {
    return { error: `Server returned ${res.status}: ${text.slice(0, ERROR_PREVIEW_CHARS)}` };
  }
}

/** The one chat in flight, and the server calls it makes. */
export class Chat {
  /** The main-process services. */
  private readonly deps: Deps;
  /** The server's API, as this browser. */
  private readonly api: ServerApi;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
  }

  /**
   * Sends the chat (and any attached files, as `data`) to the server's agent from tab `tabId` (the active one by
   * default), with control lent to it; answers its reply or `{ error }`, and refuses an incognito tab outright. `remember` false (a routine) asks the
   * server to leave the tab's chat memory alone: a routine is not a message in the person's chat, and its one
   * message would otherwise start that memory over.
   */
  send(messages: unknown[], data?: unknown, tabId?: unknown, remember = true): Promise<ChatAnswer> {
    const tab = Number.isSafeInteger(tabId) ? (tabId as number) : (this.deps.tabs.activeTabId ?? 0);
    if (this.deps.windows?.owner(tab)?.tabs.privateSession) return Promise.resolve({ error: INCOGNITO });
    if (!this.api.canCall()) return Promise.resolve({ error: OFFLINE });
    const payload = { messages, ...(data ? { data } : {}), lane: tabLane(tab), ...(remember ? {} : { remember }) };
    return this.asOnlyChat(tab, (signal) => this.inTab(tab, () => this.ask('chat', payload, signal)));
  }

  /** Runs `work` as tab `tab`'s agent with control lent to it; only a run that actually ran tells the shield it ended. */
  private inTab(tab: number, work: () => Promise<ChatAnswer>): Promise<ChatAnswer> {
    return inLane(tabLane(tab), () => this.withAgentControl(tab, work)).then((answer) => this.ended(answer));
  }

  /** Stops tab `tabId`'s chat (the latest run when no tab is named); answers whether there was one. */
  stop(tabId?: unknown): boolean {
    const run = Number.isSafeInteger(tabId) ? this.deps.chatRuns.get(tabId as number) : this.deps.chatAbort;
    run?.abort();
    return !!run;
  }

  /** Posts to this browser's route on the server; answers its JSON, or `{ error }` (STOPPED when `signal` aborted it). */
  async ask(route: string, payload: unknown, signal?: AbortSignal): Promise<ChatAnswer> {
    if (!this.api.canCall()) return { error: OFFLINE };
    try {
      return await readChatAnswer(await this.api.postToBrowser(route, payload, undefined, signal));
    } catch (err) {
      if (signal?.aborted) return { error: STOPPED };
      const message = (err as Error).message;
      return { error: message === 'terminated' ? CUT_OFF : message };
    }
  }

  /** Passes a run's answer through, telling the shield the run ended first. */
  private ended(answer: ChatAnswer): ChatAnswer {
    this.deps.shield.runEnded();
    return answer;
  }

  /**
   * Runs `work(signal)` as tab `tab`'s one chat in flight, with a signal Stop can abort.
   * Hanging up is the stop: the server sees the connection close and ends the run at its
   * next step. A second chat in the same tab, or any chat while a playbook replays, is
   * refused, as two runs would fight over the same page.
   */
  private async asOnlyChat(tab: number, work: (signal: AbortSignal) => Promise<ChatAnswer>): Promise<ChatAnswer> {
    const { chatRuns } = this.deps;
    const replaying = !!this.deps.chatAbort && ![...chatRuns.values()].includes(this.deps.chatAbort);
    if (chatRuns.has(tab) || replaying) return { error: BUSY };
    const abort = new AbortController();
    chatRuns.set(tab, abort);
    this.deps.chatAbort = abort;
    return work(abort.signal).finally(() => this.settled(tab, abort));
  }

  /** Tab `tab`'s run ended: it is no longer in flight, and the latest still running is the one Stop names. */
  private settled(tab: number, abort: AbortController): void {
    if (this.deps.chatRuns.get(tab) === abort) this.deps.chatRuns.delete(tab);
    if (this.deps.chatAbort === abort) this.deps.chatAbort = [...this.deps.chatRuns.values()].at(-1) ?? null;
  }

  /**
   * Asking this browser's agent to do something is handing it the wheel. While a
   * person held control, or automation sat paused, every command the agent sent was
   * refused ("paused for human takeover") and the run spun until it gave up. So the
   * agent gets control for the run, and a person who held it gets it back after.
   * Answers whether to take it back.
   */
  private async lendToAgent(tab: number): Promise<boolean> {
    const state = this.deps.control.snapshot();
    const mine = state.mode === 'human' && !!state.mine;
    if (mine || state.mode === 'paused') await this.deps.control.change('return');
    // Asking from a tab means "do it here": the agent drives the tab the person sent it from.
    this.deps.agentTab.claim(tab);
    return mine;
  }

  /** Runs `work` with control lent to the agent, handing it back to the person after. */
  private async withAgentControl(tab: number, work: () => Promise<ChatAnswer>): Promise<ChatAnswer> {
    const takeBack = await this.lendToAgent(tab).catch((e: Error) => e);
    if (takeBack instanceof Error) return { error: `Could not hand the browser to the agent: ${takeBack.message}` };
    try {
      return await work();
    } finally {
      this.deps.agentTab.release();
      if (takeBack) await this.deps.control.change('acquire').catch(() => {});
    }
  }
}
