/**
 * Run: a task submitted with `browser.submit()`, running in the background.
 * The polling itself lives in run-watch.ts.
 */
import type { Http } from './client.js';
import type { RunInfo, RunResult } from './types/index.js';
import { RUN_POLL_MS } from './constants.js';
import { watchRun, type RunCallbacks } from './run-watch.js';

/** A submitted task. Callbacks fire as it changes; `done` settles when it ends. */
export class Run {
  /** Settles with the result when the run succeeds, or rejects when it fails. */
  readonly done: Promise<RunResult>;

  /** Starts watching the run straight away. */
  constructor(
    private readonly http: Http,
    readonly id: string,
    callbacks: RunCallbacks,
    pollMs = RUN_POLL_MS,
  ) {
    this.done = this.watch(callbacks, pollMs);
    this.done.catch(() => {}); // callers may rely on onFailure alone
  }

  /** The run's current state. */
  status(): Promise<RunInfo> {
    return this.http.request<RunInfo>('GET', `/api/runs/${encodeURIComponent(this.id)}`);
  }

  /** Answer the open attention request: `'done'` after handling it by hand, or your reply to the agent. */
  async respond(response = 'done'): Promise<void> {
    await this.http.request('POST', `/api/runs/${encodeURIComponent(this.id)}/respond`, { response });
  }

  /** Polls until the run ends, firing callbacks on the way. */
  private watch(cb: RunCallbacks, pollMs: number): Promise<RunResult> {
    return watchRun(this, this.http.baseUrl, cb, pollMs);
  }
}
