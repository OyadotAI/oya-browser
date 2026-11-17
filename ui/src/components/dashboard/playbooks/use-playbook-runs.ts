/**
 * A playbook's run history, loaded when its row is opened: the runs, whether they
 * are still loading, why they could not be, and a retry.
 */
import { useEffect, useState } from 'react';
import { getPlaybookRuns } from './api';
import type { PlaybookRunRecord } from './types';

/** The history's state: null runs until loaded, and a failed load says so instead. */
interface RunsState {
  /** The runs, newest first, once loaded. */
  runs: PlaybookRunRecord[] | null;
  /** The last load failed. */
  failed: boolean;
}

/** Still loading. */
const LOADING: RunsState = { runs: null, failed: false };

/** Fetches the runs into `setState`; a refusal marks the load failed. */
const fetchRuns = (apiKey: string, name: string, setState: (s: RunsState) => void) =>
  getPlaybookRuns(apiKey, name).then(
    ({ runs }) => setState({ runs, failed: false }),
    () => setState({ runs: null, failed: true }),
  );

/**
 * Loads the playbook's runs on open, on Retry, and again when `lastEnded` moves:
 * the list's poll saw a newer run, so an open history shows it too.
 */
export function usePlaybookRuns(apiKey: string, name: string, lastEnded?: string) {
  const [state, setState] = useState<RunsState>(LOADING);
  useEffect(() => void fetchRuns(apiKey, name, setState), [apiKey, name, lastEnded]);
  const retry = () => (setState(LOADING), void fetchRuns(apiKey, name, setState));
  return { ...state, retry };
}
