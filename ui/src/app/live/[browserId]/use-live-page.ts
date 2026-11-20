/**
 * The live page's state: the credential, the stream and its meter, the
 * control mode, and the input sender the live view uses.
 */
'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import {
  changeControl,
  controlFor,
  openLiveView,
  sendInput,
  takeSharedCredential,
  type LiveSink,
} from './live-session';

/**
 * The credential for `browserId`: reads and strips any `#t=` fragment into tab
 * storage so reloads and React StrictMode re-runs keep the scoped share token.
 */
function useCredential(browserId: string) {
  const [apiKey, setApiKey] = useState<string | null>(null);
  const resolvedFor = useRef<string | null>(null);
  useEffect(() => {
    if (resolvedFor.current === browserId) return;
    resolvedFor.current = browserId;
    setApiKey(takeSharedCredential(browserId));
  }, [browserId]);
  return apiKey;
}

/** The picture's state: frame, fps and frame age, with stable setters. */
function usePicture() {
  const [frame, setFrame] = useState<string | null>(null);
  const [fps, setFps] = useState(0);
  const [age, setAge] = useState<number | null>(null);
  const setters = useMemo(() => ({ frame: setFrame, fps: setFps, age: setAge }), []);
  return { frame, fps, age, setters };
}

/** The stream's state and the sink that writes it. */
function useLiveFields(setMode: (mode: string) => void) {
  const [name, setName] = useState('Live browser');
  const [error, setError] = useState('');
  const { setters, ...picture } = usePicture();
  const sink = useMemo<LiveSink>(
    () => ({ ...setters, name: setName, error: setError, mode: setMode }),
    [setters, setMode],
  );
  return { name, ...picture, error, setError, sink };
}

/** The stream, reopened whenever the credential, browser or attempt changes. */
function useLiveState(apiKey: string | null, browserId: string, attempt: number, setMode: (mode: string) => void) {
  const live = useLiveFields(setMode);
  const { sink } = live;
  useEffect(() => (apiKey ? restart(browserId, apiKey, sink) : undefined), [apiKey, browserId, attempt, sink]);
  return live;
}

/** Clears the last stream's state and opens a new one. */
function restart(browserId: string, apiKey: string, sink: LiveSink) {
  sink.frame(null);
  sink.error('');
  sink.fps(0);
  sink.age(null);
  return openLiveView(browserId, apiKey, sink);
}

/** State needed to toggle human/agent control. */
interface ControlArgs {
  /** Active credential. */
  apiKey: string | null;
  /** Target browser id. */
  browserId: string;
  /** Current mode. */
  mode: string;
  /** Updates the mode. */
  setMode: (mode: string) => void;
  /** Surfaces a control error. */
  setError: (e: string) => void;
}

/** Who drives the browser, and the button that changes it. */
function useControl({ apiKey, browserId, mode, setMode, setError }: ControlArgs) {
  const toggle = () =>
    apiKey ? changeControl(browserId, apiKey, mode).then(setMode, (e) => setError(errorMessage(e))) : Promise.resolve();
  return { mode, toggle, label: controlFor(mode).label };
}

/** Shows a failed command's error on the page, then rethrows it for the live view. */
function reportAndRethrow(setError: (e: string) => void) {
  return (err: unknown): never => {
    setError(errorMessage(err));
    throw err;
  };
}

/** Sends live-view input; a failure shows on the page and is rethrown to the live view. */
function useSend(apiKey: string | null, browserId: string, setError: (e: string) => void) {
  return useCallback(
    async (action: string, params: Record<string, unknown> = {}) => {
      if (!apiKey) return;
      return sendInput(browserId, apiKey, action, params).catch(reportAndRethrow(setError));
    },
    [apiKey, browserId, setError],
  );
}

/** Everything the live page renders from. */
export function useLivePage(browserId: string) {
  const apiKey = useCredential(browserId);
  const [attempt, setAttempt] = useState(0);
  const [mode, setMode] = useState('agent');
  const live = useLiveState(apiKey, browserId, attempt, setMode);
  const control = useControl({ apiKey, browserId, mode, setMode, setError: live.setError });
  const send = useSend(apiKey, browserId, live.setError);
  return { apiKey, live, control, send, reconnect: () => setAttempt((a) => a + 1) };
}
