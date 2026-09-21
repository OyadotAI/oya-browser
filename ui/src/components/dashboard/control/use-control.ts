/**
 * The Control tab's state: data polled from the server, the action in flight
 * with its error or notice, and the Add provider form.
 */
import { useCallback, useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { errorMessage } from '@/lib/api-client';
import { CONTROL_POLL_MS } from './constants';
import { EMPTY_CONTROL, failure, loadControl, type ControlData } from './requests';
import { useProviderForm } from './use-provider-form';

/** Polled data plus the last load error. */
type DataState = ControlData & {
  /** Why the last poll failed, or ''. */
  error: string;
};

/** The action in flight and what it left behind. */
export interface ActionStatus {
  /** Label of the running action, or ''. */
  busy: string;
  /** Why the last action failed, or ''. */
  actionError: string;
  /** A success message to show, or ''. */
  notice: string;
}

/** Nothing running, nothing to say. */
const IDLE: ActionStatus = { busy: '', actionError: '', notice: '' };

/** Everything the Control tab's views read and do. */
export function useControl(apiKey: string) {
  const data = useControlData(apiKey);
  const actions = useControlActions(data.refresh);
  const form = useProviderForm(apiKey, actions);
  return { apiKey, ...data, ...actions, ...form, routing: data.fleet?.routing ?? null };
}

/** The Control tab's state and actions. */
export type Control = ReturnType<typeof useControl>;

/** Polls the key's control data while a key is set. */
export function useControlData(apiKey: string) {
  const [state, setState] = useState<DataState>({ ...EMPTY_CONTROL, error: '' });
  const refresh = useRefresh(apiKey, setState);
  usePoll(apiKey, refresh);
  return { ...state, refresh };
}

/** One load at a time: a poll that lands while another runs is skipped. */
function useRefresh(apiKey: string, setState: Dispatch<SetStateAction<DataState>>) {
  const polling = useRef(false);
  return useCallback(async () => {
    if (polling.current) return;
    polling.current = true;
    const next = await loadState(apiKey);
    setState((s) => ({ ...s, ...next }));
    polling.current = false;
  }, [apiKey, setState]);
}

/** One load: fresh data with the error cleared, or just the error. */
const loadState = (apiKey: string): Promise<Partial<DataState>> =>
  loadControl(apiKey).then(
    (data) => data,
    (e) => ({ error: errorMessage(e, 'Could not load control plane data') }),
  );

/** Loads now, then on an interval, while there is a key. */
function usePoll(apiKey: string, refresh: () => Promise<void>) {
  useEffect(() => {
    if (!apiKey) return;
    void refresh();
    const t = setInterval(() => void refresh(), CONTROL_POLL_MS);
    return () => clearInterval(t);
  }, [apiKey, refresh]);
}

/** Runs one action at a time, then reloads; failures become the action error. */
export function useControlActions(refresh: () => Promise<void>) {
  const [status, setStatus] = useState<ActionStatus>(IDLE);
  const act = useCallback(
    (label: string, fn: () => Promise<unknown>) => runAct(setStatus, refresh, label, fn),
    [refresh],
  );
  const setNotice = useCallback((notice: string) => setStatus((s) => ({ ...s, notice })), []);
  const clearError = useCallback(() => setStatus((s) => ({ ...s, actionError: '' })), []);
  return { ...status, act, setNotice, clearError };
}

/** The Control tab's action runner and its status. */
export type ControlActions = ReturnType<typeof useControlActions>;

/** Marks `label` busy, runs `fn` and a reload, then records any failure. */
async function runAct(
  setStatus: Dispatch<SetStateAction<ActionStatus>>,
  refresh: () => Promise<void>,
  label: string,
  fn: () => Promise<unknown>,
) {
  setStatus({ busy: label, actionError: '', notice: '' });
  const actionError = await failure(() => fn().then(refresh), 'Action failed');
  setStatus((s) => ({ ...s, busy: '', actionError }));
}
