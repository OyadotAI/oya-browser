/**
 * Recording from an empty fleet: start a browser with the project defaults, wait
 * for it to dial in, then start recording on it, so Record is one click even when
 * nothing is running. It reuses the run dialog's start and wait (run-steps.ts),
 * and keeps a refusal (a plan limit, a timeout) in the dialog, where Try again is.
 */
import { useEffect, useState } from 'react';
import type { BrowserRow } from '../types';
import { record } from './record-actions';
import type { Recorder } from './recorder';
import { CONNECT_TIMEOUT_MESSAGE } from './constants';
import { settlePending, startPending, type PendingBrowser } from './run-steps';

/** No profile: the server starts the browser with the project's defaults. */
const DEFAULT_PROFILE = '';

/** The start's state: in flight, waiting for a browser, ready to record on one, or why it failed. */
function useStartState() {
  const [starting, setStarting] = useState(false);
  const [pending, setPending] = useState<PendingBrowser | null>(null);
  const [ready, setReady] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  return { starting, setStarting, pending, setPending, ready, setReady, error, setError };
}

/** The start's state and setters. */
type StartState = ReturnType<typeof useStartState>;

/** Once the started browser shows up in the fleet, pick it; if it never does, say so. */
function useWaitForBrowser(s: StartState, browsers: BrowserRow[], r: Recorder) {
  const { pending, setPending, setStarting, setError, setReady } = s;
  useEffect(() => {
    if (!pending) return;
    const timeout = () => (setStarting(false), setError(CONNECT_TIMEOUT_MESSAGE));
    const start = (id: string) => (r.setBrowserId(id), setReady(id));
    settlePending(pending, browsers, { clear: () => setPending(null), start, timeout });
  }, [pending, browsers, r, setPending, setStarting, setError, setReady]);
}

/** Starts recording once the session has switched to the new browser, so it records on that one. */
function useRecordWhenPicked(s: StartState, r: Recorder) {
  const { ready, setReady, setStarting } = s;
  useEffect(() => {
    if (!ready || r.browserId !== ready) return;
    setReady(null);
    setStarting(false);
    void record(r, 'start');
  }, [ready, r, setReady, setStarting]);
}

/** Start a browser and record on it; `error` holds why the last try failed. */
export function useRecordStart(apiKey: string, browsers: BrowserRow[], r: Recorder) {
  const s = useStartState();
  useWaitForBrowser(s, browsers, r);
  useRecordWhenPicked(s, r);
  const sink = { setStarting: s.setStarting, setPending: s.setPending, toast: (m: string) => s.setError(m) };
  const start = () => (s.setError(null), startPending(apiKey, DEFAULT_PROFILE, sink));
  return { starting: s.starting, error: s.error, start };
}

/** What recording from an empty fleet gives the dialog. */
export type RecordStart = ReturnType<typeof useRecordStart>;
