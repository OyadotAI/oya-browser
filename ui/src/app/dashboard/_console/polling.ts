/**
 * The console's refresh loop: each feed on its own interval, a clock for
 * relative times, and a pause while the tab is hidden.
 */
import type { RefObject } from 'react';
import { consoleCredential } from '@/lib/api';
import { BROWSERS_POLL_MS, CLOCK_TICK_MS, FLEET_POLL_MS, PERSONAS_POLL_MS } from './constants';

/** The fetchers the loop drives. */
export interface Feeds {
  /** Refreshes the table. */
  fetchBrowsers: () => void;
  /** Refreshes the strip. */
  fetchFleet: () => void;
  /** Refreshes the personas. */
  fetchPersonas: () => void;
}

/** Polls every feed and ticks the clock; returns the stop. */
function startTimers(f: Feeds, setNow: (now: number) => void) {
  const timers = [
    setInterval(f.fetchBrowsers, BROWSERS_POLL_MS),
    setInterval(f.fetchFleet, FLEET_POLL_MS),
    setInterval(f.fetchPersonas, PERSONAS_POLL_MS),
    setInterval(() => !document.hidden && setNow(Date.now()), CLOCK_TICK_MS),
  ];
  return () => timers.forEach(clearInterval);
}

/** Pauses polls while hidden and catches up when the tab is shown again; returns the stop. */
function watchVisibility(f: Feeds, hidden: RefObject<boolean>) {
  const onVis = () => {
    hidden.current = document.hidden;
    if (document.hidden) return;
    f.fetchBrowsers();
    f.fetchFleet();
  };
  document.addEventListener('visibilitychange', onVis);
  return () => document.removeEventListener('visibilitychange', onVis);
}

/** Loads everything now, then keeps it fresh; returns the stop. */
export function startPolling(f: Feeds, hidden: RefObject<boolean>, setNow: (now: number) => void) {
  f.fetchBrowsers();
  f.fetchFleet();
  f.fetchPersonas();
  const stopTimers = startTimers(f, setNow);
  const stopWatching = watchVisibility(f, hidden);
  return () => (stopTimers(), stopWatching());
}

/** What this tab last opened. */
export interface SavedProject {
  /** The credential, or empty. */
  credential: string;
  /** The project id, or null. */
  id: string | null;
}

/** The credential and project this tab last opened, if any. */
export function savedProject(): SavedProject {
  let id: string | null = null;
  try {
    id = sessionStorage.getItem('oya_project_id');
  } catch {
    /* storage blocked */
  }
  return { credential: consoleCredential(), id };
}
