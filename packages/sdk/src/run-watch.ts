/**
 * The polling loop behind a submitted Run: it checks on the run until it
 * ends, fires the caller's callbacks as the run changes, and settles with the
 * result or the failure.
 */
import { OyaError } from './errors.js';
import { MAX_RUN_POLL_ERRORS, Status } from './constants.js';
import type { RunInfo, RunResult, SubmitOptions } from './types/index.js';

/** The callbacks a run can fire. */
export type RunCallbacks = Pick<SubmitOptions, 'onSuccess' | 'onFailure' | 'onHumanAttention' | 'onHealed'>;

/** What the loop needs from the run it watches. */
export interface RunHandle {
  /** Reads the run's current state. */
  status(): Promise<RunInfo>;
  /** Answers the open attention request. */
  respond(response?: string): Promise<void>;
}

/** One watch: the run, where its links resolve, its callbacks, and what the loop carries between polls. */
interface Watch {
  /** The run being watched. */
  run: RunHandle;
  /** Resolves a relative live-view link. */
  baseUrl: string;
  /** The caller's callbacks. */
  cb: RunCallbacks;
  /** Polls failed in a row; a success resets it. */
  errors: number;
  /** The attention request already reported, so it fires once. */
  seen?: string;
}

/** Waits `ms` milliseconds. */
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Runs a caller's callback; one that throws is logged, never allowed to break the loop. */
async function call(fn: () => unknown): Promise<void> {
  try {
    await fn();
  } catch (err) {
    console.error('[oya] run callback threw:', err);
  }
}

/** Polls until the run ends. `baseUrl` resolves a relative live-view link. */
export async function watchRun(run: RunHandle, baseUrl: string, cb: RunCallbacks, pollMs: number): Promise<RunResult> {
  const watch: Watch = { run, baseUrl, cb, errors: 0 };
  for (;;) {
    const result = await step(watch);
    if (result) return result;
    await sleep(pollMs);
  }
}

/** One poll: the result once the run has succeeded, nothing while it goes on. Throws once it failed. */
async function step(watch: Watch): Promise<RunResult | undefined> {
  const info = await poll(watch);
  if (!info) return undefined;
  noticeAttention(watch, info);
  if (info.status === 'succeeded') return succeed(watch.cb, info);
  if (info.status === 'failed') return fail(watch.cb, info);
  return undefined;
}

/** The run's state, or null after a transient failure. Too many failures in a row end the run. */
async function poll(watch: Watch): Promise<RunInfo | null> {
  try {
    const info = await watch.run.status();
    watch.errors = 0;
    return info;
  } catch (err) {
    if (++watch.errors < MAX_RUN_POLL_ERRORS) return null;
    return giveUp(watch.cb, err);
  }
}

/** The run could not be polled: report and throw that. */
async function giveUp(cb: RunCallbacks, err: unknown): Promise<never> {
  const failure = err instanceof OyaError ? err : new OyaError(String(err), 0, null);
  await call(() => cb.onFailure?.(failure));
  throw failure;
}

/** Fires onHumanAttention once per new attention request. */
function noticeAttention(watch: Watch, info: RunInfo): void {
  if (info.status !== 'needs_attention' || !info.attention || info.attention.id === watch.seen) return;
  watch.seen = info.attention.id;
  const request = {
    ...info.attention,
    liveViewUrl: info.attention.liveViewUrl && new URL(info.attention.liveViewUrl, watch.baseUrl).href,
    respond: (response?: string) => watch.run.respond(response),
  };
  // Not awaited: a handler that waits on a person must not stall polling.
  void call(() => watch.cb.onHumanAttention?.(request));
}

/** The run succeeded: onHealed first when a replay was healed, then onSuccess. */
async function succeed(cb: RunCallbacks, info: RunInfo): Promise<RunResult> {
  const result = info.result || {};
  if (result.healed) await call(() => cb.onHealed?.(result));
  await call(() => cb.onSuccess?.(result));
  return result;
}

/** The run failed: report and throw it. */
async function fail(cb: RunCallbacks, info: RunInfo): Promise<never> {
  const failure = new OyaError(info.error || 'Run failed', info.errorStatus ?? Status.SERVER_ERROR, info);
  await call(() => cb.onFailure?.(failure));
  throw failure;
}
