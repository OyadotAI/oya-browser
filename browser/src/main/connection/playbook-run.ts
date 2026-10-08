/** Supervise a server replay independently of renderer visibility and reconnects. */
import type { AppServices } from '../app/services.ts';
import type { Payload } from '../../shared/ipc.ts';
import { ServerApi, readApiAnswer } from './server-api.ts';
import { PLAYBOOK_POLL_MS } from './constants.ts';

/** Existing ownership and shared Ask/routine exclusion services. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'control' | 'recorder' | 'chatAbort'>;
/** One desktop replay keeps its project identity and ownership until it settles. */
export class PlaybookRunner {
  /** Main services, never exposed to the renderer. */
  private readonly deps: Deps;
  /** Project-scoped HTTP adapter. */
  private readonly api: ServerApi;
  /** Wire the same exclusion gate Ask and routines already use. */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
  }
  /** Submit at most once, preserving ownership when the server refuses the run. */
  async start(body: Payload): Promise<Payload> {
    this.requireIdle();
    const abort = new AbortController();
    this.deps.chatAbort = abort;
    const held = this.deps.control.snapshot().mode === 'human' && !!this.deps.control.snapshot().mine;
    return this.submit(body, abort, held).catch(async (error: unknown) => {
      await this.finish(abort, held);
      throw error;
    });
  }
  /** Shared Ask/routine ownership prevents overlapping page mutations. */
  private requireIdle(): void {
    if (this.deps.recorder.recording || this.deps.chatAbort)
      throw new Error('Finish the current recording or agent run first.');
  }
  /** Return control before starting, then supervise the background run. */
  private async submit(body: Payload, abort: AbortController, held: boolean): Promise<Payload> {
    const state = this.deps.control.snapshot();
    if (state.mode === 'human' || state.mode === 'paused') await this.deps.control.change('return');
    const run = (await readApiAnswer(await this.api.postToBrowser('runs', body))) as Payload;
    const key = this.identity();
    void this.follow(String(run.id), key, abort, held);
    return run;
  }
  /** Poll failures do not cause a second submission or falsely release an active run. */
  private async follow(id: string, key: string, abort: AbortController, held: boolean): Promise<void> {
    while (this.identity() === key) {
      await new Promise<void>((resolve) => setTimeout(resolve, PLAYBOOK_POLL_MS).unref());
      if (this.identity() !== key) break;
      const run = (await this.api.get(`runs/${encodeURIComponent(id)}`).catch(() => null)) as Payload | null;
      if (run && ['succeeded', 'failed'].includes(String(run.status))) break;
      if (abort.signal.aborted) await this.deps.control.change('acquire').catch(() => {});
    }
    await this.finish(abort, held && this.identity() === key);
  }
  /** Bind supervision to the original server, project, and browser instance. */
  private identity(): string {
    return JSON.stringify([
      this.deps.config.values.serverUrl,
      this.deps.config.values.apiKey,
      this.deps.socket.browserId,
    ]);
  }
  /** Restore a prior human hold without taking it away from somebody else. */
  private async finish(abort: AbortController, held: boolean): Promise<void> {
    if (this.deps.chatAbort === abort) this.deps.chatAbort = null;
    if (held && this.deps.control.snapshot().mode === 'agent')
      await this.deps.control.change('acquire').catch(() => {});
  }
}
