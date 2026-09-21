/**
 * The saved playbooks, refreshed on an interval, with the last load error.
 */
import { useCallback, useEffect, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { listPlaybooks } from './api';
import { PLAYBOOKS_POLL_MS } from './constants';
import type { PlaybookInfo } from './types';

/** Receives a successful load, or the reason it failed. */
interface LoadSink {
  /** The list arrived. */
  loaded: (playbooks: PlaybookInfo[]) => void;
  /** The load failed (message), or succeeded (null). */
  failed: (message: string | null) => void;
}

/** Loads the list once; a failure keeps the last list and reports why. */
async function loadPlaybooks(apiKey: string, sink: LoadSink) {
  if (!apiKey) return;
  try {
    sink.loaded((await listPlaybooks(apiKey)).playbooks);
    sink.failed(null);
  } catch (err) {
    sink.failed(errorMessage(err));
  }
}

/** Runs `refresh` now and on an interval, restarting whenever it changes. */
function usePolling(refresh: () => void) {
  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, PLAYBOOKS_POLL_MS);
    return () => clearInterval(timer);
  }, [refresh]);
}

/** The playbook list (null until first loaded), its load error, and a refresh. */
export function usePlaybookList(apiKey: string) {
  const [playbooks, setPlaybooks] = useState<PlaybookInfo[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const refresh = useCallback(() => loadPlaybooks(apiKey, { loaded: setPlaybooks, failed: setLoadError }), [apiKey]);
  usePolling(refresh);
  return { playbooks, loadError, refresh };
}
