/**
 * Ask: the conversation sent to this browser's agent on the server, one run at
 * a time, with control lent to the agent while it runs. The dev panel's chat
 * and the routines both ask through it.
 */
import type { AppServices } from '../app/services.ts';
import { ServerApi } from './server-api.ts';
import { ERROR_PREVIEW_CHARS } from './constants.ts';

/** The services a chat uses. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'control' | 'shield' | 'chatAbort'>;

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

  /** Sends the chat (and any attached files, as `data`) to the server's agent for this browser, with control lent to it; answers its reply or `{ error }`. */
  send(messages: unknown[], data?: unknown): Promise<ChatAnswer> {
    if (!this.api.canCall()) return Promise.resolve({ error: OFFLINE });
    const payload = data ? { messages, data } : { messages };
    // Only a run that actually ran tells the shield it ended; a chat refused while another runs leaves that one's show alone.
    const run = (signal: AbortSignal): Promise<ChatAnswer> =>
      this.withAgentControl(() => this.ask('chat', payload, signal)).then((answer) => this.ended(answer));
    return this.asOnlyChat(run);
  }

  /** Stops the chat in flight, if any; answers whether there was one. */
  stop(): boolean {
    this.deps.chatAbort?.abort();
    return !!this.deps.chatAbort;
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
   * Runs `work(signal)` as the one chat in flight, with a signal Stop can abort.
   * Hanging up is the stop: the server sees the connection close and ends the run
   * at its next step. A second chat (or a routine) while one runs is refused, as
   * two runs would fight over the same page.
   */
  private async asOnlyChat(work: (signal: AbortSignal) => Promise<ChatAnswer>): Promise<ChatAnswer> {
    if (this.deps.chatAbort) return { error: BUSY };
    const abort = (this.deps.chatAbort = new AbortController());
    try {
      return await work(abort.signal);
    } finally {
      if (this.deps.chatAbort === abort) this.deps.chatAbort = null;
    }
  }

  /**
   * Asking this browser's agent to do something is handing it the wheel. While a
   * person held control, or automation sat paused, every command the agent sent was
   * refused ("paused for human takeover") and the run spun until it gave up. So the
   * agent gets control for the run, and a person who held it gets it back after.
   * Answers whether to take it back.
   */
  private async lendToAgent(): Promise<boolean> {
    const state = this.deps.control.snapshot();
    const mine = state.mode === 'human' && !!state.mine;
    if (mine || state.mode === 'paused') await this.deps.control.change('return');
    return mine;
  }

  /** Runs `work` with control lent to the agent, handing it back to the person after. */
  private async withAgentControl(work: () => Promise<ChatAnswer>): Promise<ChatAnswer> {
    const takeBack = await this.lendToAgent().catch((e: Error) => e);
    if (takeBack instanceof Error) return { error: `Could not hand the browser to the agent: ${takeBack.message}` };
    try {
      return await work();
    } finally {
      if (takeBack) await this.deps.control.change('acquire').catch(() => {});
    }
  }
}
