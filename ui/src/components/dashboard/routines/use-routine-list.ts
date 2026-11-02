/**
 * The project's routines, refreshed on an interval, with the last load error and
 * a way to put a change's answer into its row at once instead of waiting a poll.
 */
import { useCallback, useState, type Dispatch, type SetStateAction } from 'react';
import { errorMessage } from '@/lib/api-client';
import { usePolling } from '@/lib/hooks/use-polling';
import { listRoutines } from './api';
import { ROUTINES_POLL_MS } from './constants';
import type { Routine } from './types';

/** Receives a successful load, or the reason it failed. */
interface LoadSink {
  /** The list arrived. */
  loaded: (routines: Routine[]) => void;
  /** The load failed (message), or succeeded (null). */
  failed: (message: string | null) => void;
}

/** Loads the list once; a failure keeps the last list and reports why. */
async function loadRoutines(apiKey: string, sink: LoadSink) {
  if (!apiKey) return;
  try {
    sink.loaded((await listRoutines(apiKey)).routines);
    sink.failed(null);
  } catch (err) {
    sink.failed(errorMessage(err));
  }
}

/** `list` with `routine` in its row, or added at the end when it is new. */
export function withRoutine(list: Routine[] | null, routine: Routine): Routine[] {
  const rows = list || [];
  if (!rows.some((r) => r.id === routine.id)) return [...rows, routine];
  return rows.map((r) => (r.id === routine.id ? routine : r));
}

/**
 * Counts changes to the shown list made from a save's answer, so a load can tell
 * one landed while it was out and drop its older answer: a slow poll cannot put
 * the old routine back for a whole interval.
 */
class ChangeCount {
  /** Changes so far. */
  private count = 0;

  /** Notes one change. */
  bump() {
    this.count += 1;
  }

  /** Changes so far. */
  now() {
    return this.count;
  }
}

/** Sets the shown list. */
type SetList = Dispatch<SetStateAction<Routine[] | null>>;

/** Changes to the shown list from a save's answer, each counted. */
function useEdits(setRoutines: SetList, changes: ChangeCount) {
  const change = useCallback(
    (edit: (list: Routine[] | null) => Routine[]) => (changes.bump(), setRoutines(edit)),
    [setRoutines, changes],
  );
  const replace = useCallback((routine: Routine) => change((list) => withRoutine(list, routine)), [change]);
  const drop = useCallback((id: string) => change((list) => (list || []).filter((r) => r.id !== id)), [change]);
  return { replace, drop };
}

/** The routine list (null until first loaded), its load error, a refresh, and edits to the shown list. */
export function useRoutineList(apiKey: string) {
  const [routines, setRoutines] = useState<Routine[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [changes] = useState(() => new ChangeCount());
  const refresh = useCallback(() => loadFresh(apiKey, changes, setRoutines, setLoadError), [apiKey, changes]);
  usePolling(refresh, ROUTINES_POLL_MS);
  return { routines, loadError, refresh, ...useEdits(setRoutines, changes) };
}

/** Loads the list, showing the answer only if no save changed the list meanwhile. */
function loadFresh(apiKey: string, changes: ChangeCount, setRoutines: SetList, failed: (e: string | null) => void) {
  const seen = changes.now();
  const loaded = (routines: Routine[]) => changes.now() === seen && setRoutines(routines);
  return loadRoutines(apiKey, { loaded, failed });
}

/** What the list hook gives the rest of the tab. */
export type RoutineList = ReturnType<typeof useRoutineList>;
