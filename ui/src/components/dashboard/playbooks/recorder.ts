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
  const refs = useSessionRefs(useLatest(apiKey), browserId);
  return { apiKey, toast, browserId, setBrowserId, state, setState, busy, setBusy, held, setHeld, ...refs };
}

/** The session's refs: holds control, still open, and the start/stop counter. */
type SessionRefs = Pick<Recorder, 'acquired' | 'mounted' | 'revision'>;

/** The dialog is going away: if it holds control, stop capture and hand it back, under the credential of that moment. */
function releaseOnExit(key: RefObject<string>, browserId: string, { mounted, acquired }: SessionRefs) {
  mounted.current = false;
  if (acquired.current) void recordCall(key.current, browserId, STOP_AND_RESUME).catch(() => undefined);
}

/** The latest credential, for a cleanup that runs long after the render that made it. */
function useLatest(apiKey: string) {
  const latest = useRef(apiKey);
  useEffect(() => void (latest.current = apiKey), [apiKey]);
  return latest;
}

/**
 * The session's refs. Stop capture and hand control back on every exit, including
 * navigation away from this tab. The server retains the final steps for recovery.
 * The console renews its credential every 45 minutes: that is not an exit, and
 * treating it as one stopped every long recording, so the exit reads the key from a ref.
 */
function useSessionRefs(key: RefObject<string>, browserId: string): SessionRefs {
  const acquired = useRef(false);
  const mounted = useRef(true);
  const revision = useRef(0);
  useEffect(() => {
    mounted.current = true;
    return () => releaseOnExit(key, browserId, { acquired, mounted, revision });
  }, [browserId, key]);
  return { acquired, mounted, revision };
}
