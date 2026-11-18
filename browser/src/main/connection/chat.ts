/**
 * Ask: the conversation sent to this browser's agent on the server, one run per
 * tab at a time, with control lent to the agent while it runs. Each tab's chat is
 * its own agent (lane `tab-<id>`), driving that tab beside the other tabs' agents.
 * The panel and the routines both ask through it.
 */
import { randomUUID } from 'node:crypto';
import { setTimeout as sleep } from 'node:timers/promises';
import { inLane } from '../control/agent-tab.ts';
import type { AppServices } from '../app/services.ts';
import { ServerApi } from './server-api.ts';
import {
  CHAT_STOP_TIMEOUT_MS,
  ERROR_PREVIEW_CHARS,
  HttpStatus,
  REATTACH_RETRY_MS,
  REATTACH_WAIT_MS,
} from './constants.ts';

/** The services a chat uses. */
type Deps = Pick<
  AppServices,
  'config' | 'socket' | 'control' | 'shield' | 'shell' | 'chatAbort' | 'chatRuns' | 'agentTab' | 'tabs' | 'windows'
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
/**
 * What a chat answers when its connection to the server dropped mid-run (fetch says "terminated")
 * and the run could not be collected afterwards: the window to come back in passed, or the server
 * no longer has the run (it restarted, or it is too old to keep one).
 */
const CUT_OFF =
  'The connection to the server dropped and the run could not be resumed. Check where the page got to, then ask again.';
/** One try at collecting a run found the server has no such run: there is nothing to wait for. */
const GONE = Symbol('gone');
/** One try at collecting a run was cut like the request before it: the run is still there, so the wait starts over. */
const CUT = Symbol('cut');
/** One try at collecting a run could not reach the server: try again shortly. */
const RETRY = Symbol('retry');
/** How one try at collecting a run ended. */
type Collected = ChatAnswer | typeof GONE | typeof CUT | typeof RETRY;

/** What a chat sends the server. */
interface Sent {
  /** The tab's lane. */
  lane: string;
  /** Names this run, so the desktop can collect its answer after a dropped connection, and never another run's. */
  resume: string;
  /** The conversation, the files and whether it is remembered. */
  [key: string]: unknown;
}

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
    const said = { messages, ...(data ? { data } : {}), lane: tabLane(tab), ...(remember ? {} : { remember }) };
    const payload: Sent = { ...said, resume: randomUUID() };
    return this.asOnlyChat(tab, (signal) => this.inTab(tab, () => this.converse(tab, payload, signal)));
  }

  /**
   * Asks, and when the request is cut mid-run comes back for the same run: a network that drops a
   * long request used to end the run. A run this side has given up on (stopped, failed to reach,
   * or not collected) is told to stop, since hanging up no longer stops a run that named a token.
   */
  private async converse(tab: number, payload: Sent, signal: AbortSignal): Promise<ChatAnswer> {
    const first = await this.ask('chat', payload, signal);
    const answer = first.error === CUT_OFF ? await this.reattach(tab, payload.resume, signal) : first;
    if (signal.aborted || answer.error) void this.stopOnServer(tab, payload.resume);
    return answer;
  }

  /**
   * Collects the run's answer, trying until the window the server keeps an unheard run for has
   * passed. Over HTTP alone, not waiting for the socket: the server holds the run through the
   * socket's gap. A try cut like the first request means the run is still going, so the wait starts over.
   */
  private async reattach(tab: number, token: string, signal: AbortSignal): Promise<ChatAnswer> {
    let deadline = Date.now() + REATTACH_WAIT_MS;
    while (!signal.aborted && Date.now() < deadline) {
      const got = await this.collect(tab, token, signal);
      if (typeof got === 'object') return got;
      if (got === GONE) break;
      deadline = await this.nextDeadline(got === CUT, deadline, signal);
    }
    return { error: signal.aborted ? STOPPED : CUT_OFF };
  }

  /** When to give up after a failed try: a cut try starts the wait over at once, any other pauses and keeps the deadline. */
  private async nextDeadline(cut: boolean, deadline: number, signal: AbortSignal): Promise<number> {
    if (cut) return Date.now() + REATTACH_WAIT_MS;
    await sleep(REATTACH_RETRY_MS, undefined, { signal }).catch(() => {});
    return deadline;
  }

  /**
   * One try at collecting the run, with the panel told it is reconnecting; a try that fails says how.
   * Only a try that listened for a while counts as cut: one cut at once is a server or proxy that is
   * not holding the request at all, and counting it would start the wait over forever.
   */
  private collect(tab: number, token: string, signal: AbortSignal): Promise<Collected> {
    this.tell(tab, 'reconnecting');
    const began = Date.now();
    const listened = () => Date.now() - began >= REATTACH_RETRY_MS;
    const failed = (err: Error): Collected => (err.message === 'terminated' && listened() ? CUT : RETRY);
    return this.resumed(tab, token, signal).catch(failed);
  }

  /** Asks the server for the run's answer: the answer once the run ends, or GONE or RETRY when the server refuses. */
  private async resumed(tab: number, token: string, signal: AbortSignal): Promise<Collected> {
    const body = { lane: tabLane(tab), resume: token };
    const res = await this.api.postToBrowser('chat/resume', body, undefined, signal);
    if (!res.ok) return res.status === HttpStatus.NOT_FOUND ? GONE : RETRY;
    this.tell(tab, 'resumed');
    return readChatAnswer(res);
  }

  /** Tells tab `tab`'s panel how its run's connection stands, as an agent event of no run. */
  private tell(tab: number, kind: 'reconnecting' | 'resumed'): void {
    this.deps.shell.send('agent-event', { runId: '', event: { kind }, tabId: tab });
  }

  /**
   * Tells the server to stop the run named `token`, trying until the server hears it or the window
   * it keeps an unheard run for has passed. Stop pressed while the network is down would otherwise
   * be lost, and the run would drive the browser until the server gave up on it. Naming the run
   * keeps a stop that lands late from ending the next run in the same tab.
   */
  private async stopOnServer(tab: number, token: string): Promise<void> {
    const until = Date.now() + REATTACH_WAIT_MS;
    while (!(await this.heardStop(tab, token)) && Date.now() < until)
      await sleep(REATTACH_RETRY_MS, undefined, { ref: false });
  }

  /** One try at telling the server to stop the run; any answer at all means it heard. */
  private heardStop(tab: number, token: string): Promise<boolean> {
    const body = { lane: tabLane(tab), resume: token };
    const heard = this.api.postToBrowser('chat/stop', body, CHAT_STOP_TIMEOUT_MS);
    return heard.then(
      () => true,
      () => false,
    );
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
