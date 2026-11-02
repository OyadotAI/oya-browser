/**
 * What a row's buttons do: turn a routine on or off, run a cloud one now, clear
 * its history, and delete it after asking. A 404 means someone deleted it
 * elsewhere; Run now's 409 goes into the row's note, since it is about that row.
 */
import { ApiError, errorMessage } from '@/lib/api-client';
import { Status } from '@/lib/http-status';
import { usePatchState, type Patch } from '@/lib/hooks/use-patch-state';
import { useToast } from '@/components/dashboard/toast';
import { clearRuns, deleteRoutine, editRoutine, runNow } from './api';
import { NOTE_MS, ROUTINES_TEXT } from './constants';
import type { RoutineList } from './use-routine-list';
import type { Routine } from './types';

/** The rows' in-flight state. */
export interface ActionState {
  /** A switch flipped and not yet saved, by routine id: what it now shows. */
  pending: Record<string, boolean>;
  /** Routines whose Run now is in flight. */
  starting: Record<string, boolean>;
  /** A row's note, by routine id. */
  notes: Record<string, string>;
  /** The routine waiting for its delete to be confirmed. */
  confirming: Routine | null;
  /** The delete is in flight. */
  deleting: boolean;
}

/** Nothing in flight. */
const IDLE: ActionState = { pending: {}, starting: {}, notes: {}, confirming: null, deleting: false };

/** `record` without `id`. */
const without = <T>(record: Record<string, T>, id: string) =>
  Object.fromEntries(Object.entries(record).filter(([key]) => key !== id));

/** Whether a request failed because the routine is gone. */
const isGone = (err: unknown) => err instanceof ApiError && err.status === Status.NOT_FOUND;

/** Shows a failed request: a routine gone elsewhere refreshes the list, anything else says why. */
function failed(err: unknown, list: RoutineList, toast: ReturnType<typeof useToast>) {
  if (!isGone(err)) return toast(errorMessage(err), 'error');
  toast(ROUTINES_TEXT.gone, 'error');
  list.refresh();
}

/** Puts `note` on a row for NOTE_MS. */
function noteOn(patch: Patch<ActionState>, id: string, note: string) {
  patch((s) => ({ notes: { ...s.notes, [id]: note } }));
  setTimeout(() => patch((s) => ({ notes: s.notes[id] === note ? without(s.notes, id) : s.notes })), NOTE_MS);
}

/** What the actions need: the key, the list, the state and how to say something. */
interface Deps {
  /** The caller's API key. */
  apiKey: string;
  /** The routine list. */
  list: RoutineList;
  /** Merges into the action state. */
  patch: Patch<ActionState>;
  /** Raises a toast. */
  toast: ReturnType<typeof useToast>;
}

/** Runs `work`; a failure `handle` does not take is shown by `failed`. */
async function attempt(deps: Deps, work: () => Promise<void>, handle: (err: unknown) => boolean = () => false) {
  try {
    await work();
  } catch (err) {
    if (!handle(err)) failed(err, deps.list, deps.toast);
  }
}

/** Turns a routine on or off; the switch shows the new state until the save settles. */
async function toggle(deps: Deps, routine: Routine) {
  const { apiKey, list, patch } = deps;
  patch((s) => ({ pending: { ...s.pending, [routine.id]: !routine.enabled } }));
  await attempt(deps, async () => list.replace(await editRoutine(apiKey, routine.id, { enabled: !routine.enabled })));
  patch((s) => ({ pending: without(s.pending, routine.id) }));
}

/** Takes Run now's 409 into the row's note; answers whether it did. */
const conflictNote = (patch: Patch<ActionState>, id: string) => (err: unknown) => {
  if (!(err instanceof ApiError) || err.status !== Status.CONFLICT) return false;
  noteOn(patch, id, err.message);
  return true;
};

/** Runs a cloud routine now; a refusal (409) goes into the row's note. */
async function start(deps: Deps, routine: Routine) {
  const { apiKey, list, patch } = deps;
  patch((s) => ({ starting: { ...s.starting, [routine.id]: true }, notes: without(s.notes, routine.id) }));
  const work = async () => {
    list.replace(await runNow(apiKey, routine.id));
    noteOn(patch, routine.id, ROUTINES_TEXT.started);
  };
  await attempt(deps, work, conflictNote(patch, routine.id));
  patch((s) => ({ starting: without(s.starting, routine.id) }));
}

/** Clears a routine's finished runs. */
const clear = (deps: Deps, routine: Routine) =>
  attempt(deps, async () => deps.list.replace(await clearRuns(deps.apiKey, routine.id)));

/** Deletes the routine waiting for confirmation. */
async function remove(deps: Deps, routine: Routine) {
  deps.patch({ deleting: true });
  await attempt(deps, async () => {
    await deleteRoutine(deps.apiKey, routine.id);
    deps.list.drop(routine.id);
    deps.toast(ROUTINES_TEXT.deleted(routine.name), 'success');
  });
  deps.patch({ deleting: false, confirming: null });
}

/** The row handlers bound to `deps` and the action state. */
const handlersOf = (deps: Deps, state: ActionState) => ({
  toggle: (r: Routine) => toggle(deps, r),
  runNow: (r: Routine) => start(deps, r),
  clear: (r: Routine) => clear(deps, r),
  askDelete: (r: Routine | null) => deps.patch({ confirming: r }),
  confirmDelete: () => state.confirming && remove(deps, state.confirming),
});

/** The rows' state and what their buttons do. */
export function useRoutineActions(apiKey: string, list: RoutineList) {
  const [state, patch] = usePatchState<ActionState>(IDLE);
  const toast = useToast();
  return { state, ...handlersOf({ apiKey, list, patch, toast }, state) };
}

/** What the action hook gives the rows. */
export type RoutineActions = ReturnType<typeof useRoutineActions>;
