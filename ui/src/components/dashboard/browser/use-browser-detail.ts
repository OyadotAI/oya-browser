/**
 * The open browser's detail and control mode, polled while the panel is open,
 * plus the optimistic activity lines shown until the server catches up.
 */
import { useCallback, useEffect, useState } from 'react';
import { api } from '@/lib/api-client';
import type { BrowserDetail } from '../types';
import type { OptimisticLine } from './types';
import { OPTIMISTIC_MAX, OPTIMISTIC_TTL_MS, POLL_MS, Status } from './constants';

/** What GET /control/sessions/:id answers, as far as the panel reads it. */
interface Session {
  /** Who drives the browser. */
  control: {
    /** 'agent', 'human' or 'paused'. */
    mode: string;
  };
}

/** An error that may carry an HTTP status. */
interface WithStatus {
  /** The HTTP status, when the error came from the API. */
  status?: number;
}

/** Where a refresh writes what it read. */
interface Sinks {
  /** Stores the detail. */
  setDetail: (d: BrowserDetail) => void;
  /** Stores the control mode. */
  setControlMode: (mode: string) => void;
  /** Drops optimistic lines the server has had time to log. */
  settle: () => void;
}

/** The lines still young enough that the server may not have logged them yet. */
export const freshLines = (lines: OptimisticLine[], now = Date.now()) =>
  lines.filter((x) => now - new Date(x.ts).getTime() < OPTIMISTIC_TTL_MS);

/** Adds a line at the top, keeping only the newest few. */
export const pushLine = (lines: OptimisticLine[], line: string) =>
  [{ ts: new Date().toISOString(), line }, ...lines].slice(0, OPTIMISTIC_MAX);

/** One poll: the detail first, then the control mode (a missing session is not an error). */
async function refreshOnce(browserId: string, apiKey: string, s: Sinks) {
  s.setDetail(await api<BrowserDetail>(`/browsers/${browserId}`, { key: apiKey }));
  const session = await api<Session>(`/control/sessions/${browserId}`, { key: apiKey }).catch(() => null);
  if (session) s.setControlMode(session.control.mode);
  // The server has caught up with whatever we did; drop the placeholders.
  s.settle();
}

/** The optimistic activity lines: `onInput` adds one, `settle` drops the stale ones. */
function useOptimistic() {
  const [optimistic, setOptimistic] = useState<OptimisticLine[]>([]);
  const onInput = useCallback((line: string) => setOptimistic((o) => pushLine(o, line)), []);
  const settle = useCallback(() => setOptimistic((o) => freshLines(o)), []);
  return { optimistic, onInput, settle };
}

/** Closes the panel when a poll says the browser is gone; other errors wait for the next poll. */
const closeIfGone = (onClose: () => void) => (err: WithStatus) => {
  if (err.status === Status.NOT_FOUND) onClose();
};

/** Polls every POLL_MS; a 404 means the browser is gone, so the panel closes. */
function useRefresh(browserId: string, apiKey: string, onClose: () => void, sinks: Sinks) {
  const { setDetail, setControlMode, settle } = sinks;
  const refresh = useCallback(
    () => refreshOnce(browserId, apiKey, { setDetail, setControlMode, settle }).catch(closeIfGone(onClose)),
    [browserId, apiKey, onClose, setDetail, setControlMode, settle],
  );
  usePolling(refresh);
  return refresh;
}

/** Runs `refresh` now and every POLL_MS until it changes or the panel closes. */
function usePolling(refresh: () => void) {
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, POLL_MS);
    return () => clearInterval(t);
  }, [refresh]);
}

/** The detail, control mode and optimistic lines, kept fresh while the panel is open. */
export function useBrowserDetail(browserId: string, apiKey: string, onClose: () => void) {
  const [detail, setDetail] = useState<BrowserDetail | null>(null);
  const [controlMode, setControlMode] = useState('agent');
  const { optimistic, onInput, settle } = useOptimistic();
  const refresh = useRefresh(browserId, apiKey, onClose, { setDetail, setControlMode, settle });
  return { detail, controlMode, setControlMode, optimistic, onInput, refresh };
}
