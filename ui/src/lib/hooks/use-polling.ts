/**
 * Runs a refresh now and then on an interval, for lists the server owns and the
 * console only shows: the playbooks, the routines.
 */
'use client';

import { useEffect } from 'react';

/** Calls `refresh` now and every `ms`, restarting whenever either changes. */
export function usePolling(refresh: () => void, ms: number) {
  useEffect(() => {
    refresh();
    const timer = setInterval(refresh, ms);
    return () => clearInterval(timer);
  }, [refresh, ms]);
}
