/**
 * Reading a recording's status: once on open, to find a stopped flow, and on
 * a loop while capture runs.
 */
import { errorMessage } from '@/lib/api-client';
import type { useToast } from '../toast';
import { postInput, recordCall, type InputResult } from './api';
import { RECORD_POLL_MS } from './constants';
import type { RecordState } from './types';

/** Shows a toast. */
type Toast = ReturnType<typeof useToast>;

/** Which browser, as whom. */
export interface Target {
  /** The caller's API key. */
  apiKey: string;
  /** The browser. */
  browserId: string;
}

/** Where status answers go. */
interface StatusSink {
  /** A fresh status. */
  state: (s: RecordState) => void;
  /** Why a status request failed. */
  error: (message: string) => void;
}

/** A poll loop's state, shared with its stop. */
interface PollLoop {
  /** Stopped: drop answers and schedule nothing more. */
  cancelled: boolean;
  /** The next scheduled request. */
  timer?: ReturnType<typeof setTimeout>;
}

/** Runs `then` with the promise's value unless the returned cleanup ran first. */
export function whenLive<T>(promise: Promise<T>, then: (value: T) => void) {
  let cancelled = false;
  void promise.then((value) => {
    if (!cancelled) then(value);
  });
  return () => void (cancelled = true);
}

/** A stopped recording with steps, to reopen; an active flow is rejoined through Start, which acquires control. */
export async function restoreStopped(apiKey: string, browserId: string): Promise<RecordState | null> {
  try {
    const saved = await recordCall<RecordState>(apiKey, browserId, { mode: 'status' });
    return !saved.recording && saved.steps.length ? saved : null;
  } catch {
    return null;
  }
}

/** One status request; answers after a stop are dropped. */
async function fetchStatus({ apiKey, browserId }: Target, loop: PollLoop, sink: StatusSink) {
  try {
    const next = await recordCall<RecordState>(apiKey, browserId, { mode: 'status' });
    if (!loop.cancelled) sink.state(next);
  } catch (err) {
    if (!loop.cancelled) sink.error(errorMessage(err));
  }
}

/** Ends a poll loop. */
function stopLoop(loop: PollLoop) {
  loop.cancelled = true;
  clearTimeout(loop.timer);
}

/** Polls status one request at a time, each after the last answered; returns the stop. */
export function pollStatus(target: Target, sink: StatusSink) {
  const loop: PollLoop = { cancelled: false };
  const tick = async () => {
    await fetchStatus(target, loop, sink);
    if (!loop.cancelled) loop.timer = setTimeout(tick, RECORD_POLL_MS);
  };
  loop.timer = setTimeout(tick, RECORD_POLL_MS);
  return () => stopLoop(loop);
}

/** Sends one input action; a refusal (or failed request) is toasted and returned as `ok: false`. */
export async function sendAction(t: Target, toast: Toast, action: string, params: Record<string, unknown>) {
  const r: InputResult = await postInput(t.apiKey, t.browserId, action, params).catch((err) => ({
    ok: false,
    error: errorMessage(err),
  }));
  if (r.ok === false) toast(r.error || `${action} failed`, 'error');
  return r;
}
