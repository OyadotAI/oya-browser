/**
 * The run dialog's requests and their outcomes: follow a run, start one, start
 * a browser to run on, and answer a paused run.
 */
import { errorMessage } from '@/lib/api-client';
import type { useToast } from '../toast';
import type { BrowserRow } from '../types';
import { getRun, respondToRun, startBrowser, startRun, type RunRequest } from './api';
import { CONNECT_TIMEOUT_MS, POLL_FAILURES_BEFORE_NOTICE, RUN_POLL_MS } from './constants';
import { isEnded } from './format';
import type { RunInfo } from './types';

/** Shows a toast. */
type Toast = ReturnType<typeof useToast>;

/** A browser started from here, and when to stop waiting for it. */
export interface PendingBrowser {
  /** Its id, to spot it in the fleet list. */
  id: string;
  /** Give up after this time (ms since the epoch). */
  until: number;
}

/** What following a run reports to. */
interface Follow {
  /** Shows the run's latest state. */
  setRun: (r: RunInfo) => void;
  /** The run ended. */
  onFinished: () => void;
  /** Contact with the run is lost: said once per outage, not once per poll. */
  onTrouble: (message: string) => void;
  /** Polls failed in a row. */
  failures: number;
}

/** One failed poll; the third in a row is said out loud, because a run that stops updating looks stuck. */
function pollFailed(follow: Follow, err: unknown) {
  follow.failures++;
  if (follow.failures === POLL_FAILURES_BEFORE_NOTICE)
    follow.onTrouble(`Lost contact with the run (${errorMessage(err)}). Still trying.`);
}

/** One poll of a run; a failed poll is counted and the next one tries again. */
async function pollRun(apiKey: string, runId: string, follow: Follow) {
  try {
    const next = await getRun(apiKey, runId);
    follow.failures = 0;
    follow.setRun(next);
    if (isEnded(next.status)) follow.onFinished();
  } catch (err) {
    pollFailed(follow, err);
  }
}

/** What a caller following a run hands over; it may have nowhere to report lost contact. */
export type RunSinks = Pick<Follow, 'setRun' | 'onFinished'> & Partial<Pick<Follow, 'onTrouble'>>;

/** Polls a run until it ends; returns the stop. `sinks.onTrouble` hears when contact is lost. */
export function followRun(apiKey: string, runId: string, sinks: RunSinks) {
  const follow: Follow = { onTrouble: () => undefined, ...sinks, failures: 0 };
  const timer = setInterval(() => void pollRun(apiKey, runId, follow), RUN_POLL_MS);
  return () => clearInterval(timer);
}

/** Keeps the browser choice inside the profile filter, falling back to its first browser. */
export function keepInFilter(matching: BrowserRow[], browserId: string, setBrowserId: (id: string) => void) {
  if (!matching.some((b) => b.id === browserId)) setBrowserId(matching[0]?.id || '');
}

/** Starts a browser on `persona` and waits for it; a refusal is toasted and ends the start. */
export async function startPending(apiKey: string, persona: string, sink: PendingSink) {
  sink.setStarting(true);
  try {
    const { id } = await startBrowser(apiKey, persona);
    sink.setPending({ id, until: Date.now() + CONNECT_TIMEOUT_MS });
  } catch (err) {
    sink.toast(errorMessage(err), 'error');
    sink.setStarting(false);
  }
}

/** Where starting a browser to run on reports. */
export interface PendingSink {
  /** Whether a start is in flight. */
  setStarting: (s: boolean) => void;
  /** The browser to wait for. */
  setPending: (p: PendingBrowser) => void;
  /** Reports a failure. */
  toast: Toast;
}

/** Starts a run on `browserId`; a refusal is toasted. */
export async function launchRun(apiKey: string, browserId: string, body: RunRequest, sink: RunSink) {
  sink.setStarting(true);
  try {
    sink.setRun(await startRun(apiKey, browserId, body));
  } catch (err) {
    sink.toast(errorMessage(err), 'error');
  } finally {
    sink.setStarting(false);
  }
}

/** Where a run's start reports. */
export interface RunSink {
  /** The started run. */
  setRun: (r: RunInfo) => void;
  /** Whether a start is in flight. */
  setStarting: (s: boolean) => void;
  /** Reports a failure. */
  toast: Toast;
}

/** What to do once the pending browser shows up, or its time runs out. */
export interface PendingOutcome {
  /** Clears the wait. */
  clear: () => void;
  /** Runs on the browser that dialled in. */
  start: (id: string) => void;
  /** Gives up. */
  timeout: () => void;
}

/**
 * The dashboard already polls GET /browsers every 3s, so watching the list is the
 * whole wait, no second poller, and it ends on its own deadline.
 */
export function settlePending(pending: PendingBrowser, browsers: BrowserRow[], out: PendingOutcome) {
  if (browsers.some((b) => b.id === pending.id)) {
    out.clear();
    out.start(pending.id);
  } else if (Date.now() > pending.until) {
    out.clear();
    out.timeout();
  }
}

/** Answers a paused run ("done" when the reply is blank) and marks it running again. */
export async function answerRun(apiKey: string, run: RunInfo, reply: string, sink: ReplySink) {
  try {
    await respondToRun(apiKey, run.id, reply.trim() || 'done');
    sink.setReply('');
    sink.setRun({ ...run, status: 'running', attention: null });
  } catch (err) {
    sink.toast(errorMessage(err), 'error');
  }
}

/** Where a reply's outcome goes. */
export interface ReplySink {
  /** Clears the reply box. */
  setReply: (s: string) => void;
  /** Updates the run. */
  setRun: (r: RunInfo) => void;
  /** Reports a failure. */
  toast: Toast;
}
