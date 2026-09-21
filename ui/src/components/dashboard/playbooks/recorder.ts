/**
 * The recording session's shared state, as one object the recorder's effects
 * and actions take.
 */
import { useEffect, useRef, useState, type RefObject } from 'react';
import { useToast } from '../toast';
import { recordCall, STOP_AND_RESUME } from './api';
import type { RecordBusy, RecordState } from './types';

/** Everything a recording session keeps between renders. */
export interface Recorder {
  /** The caller's API key. */
  apiKey: string;
  /** Reports failures. */
  toast: ReturnType<typeof useToast>;
  /** The browser being recorded. */
  browserId: string;
  /** Picks the browser, before recording starts. */
  setBrowserId: (id: string) => void;
  /** The server's view of the recording; null until one starts or is found. */
  state: RecordState | null;
  /** Replaces it. */
  setState: (s: RecordState) => void;
  /** Which request is in flight. */
  busy: RecordBusy;
  /** Marks one in flight. */
  setBusy: (b: RecordBusy) => void;
  /** Someone else holds the browser, so Start offers to take over. */
  held: boolean;
  /** Sets it. */
  setHeld: (h: boolean) => void;
  /** This dialog holds control, so leaving must hand it back. */
  acquired: RefObject<boolean>;
  /** The dialog is still open. */
  mounted: RefObject<boolean>;
  /** Bumped by every start and stop, so a stale status answer is ignored. */
  revision: RefObject<number>;
}

/** Creates the session's state; the first running browser is picked. */
export function useRecorderState(apiKey: string, firstBrowser: string): Recorder {
  const toast = useToast();
  const [browserId, setBrowserId] = useState(firstBrowser);
  const [state, setState] = useState<RecordState | null>(null);
  const [busy, setBusy] = useState<RecordBusy>(null);
  const [held, setHeld] = useState(false);
  const refs = useSessionRefs(apiKey, browserId);
  return { apiKey, toast, browserId, setBrowserId, state, setState, busy, setBusy, held, setHeld, ...refs };
}

/** The dialog is going away: if it holds control, stop capture and hand it back. */
function releaseOnExit(apiKey: string, browserId: string, mounted: RefObject<boolean>, acquired: RefObject<boolean>) {
  mounted.current = false;
  if (acquired.current) void recordCall(apiKey, browserId, STOP_AND_RESUME).catch(() => undefined);
}

/**
 * The session's refs. Stop capture and hand control back on every exit, including
 * navigation away from this tab. The server retains the final steps for recovery.
 */
function useSessionRefs(apiKey: string, browserId: string) {
  const acquired = useRef(false);
  const mounted = useRef(true);
  const revision = useRef(0);
  useEffect(() => {
    mounted.current = true;
    return () => releaseOnExit(apiKey, browserId, mounted, acquired);
  }, [browserId, apiKey]);
  return { acquired, mounted, revision };
}
