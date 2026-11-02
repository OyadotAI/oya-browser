/**
 * Every request the Routines tab makes, in one place: the project's routines,
 * their edits, Run now and clearing a history.
 */
import { api } from '@/lib/api-client';
import type { Routine, RoutineBody } from './types';

/** A path segment, escaped. */
const seg = encodeURIComponent;

/** What GET /routines answers. */
export interface RoutineList {
  /** The project's routines. */
  routines: Routine[];
}

/** The project's routines with their history. */
export const listRoutines = (key: string) => api<RoutineList>('/routines', { key });

/** Adds a routine. */
export const createRoutine = (key: string, body: RoutineBody) =>
  api<Routine>('/routines', { key, method: 'POST', body });

/** Changes some of a routine's fields; its history is kept. */
export const editRoutine = (key: string, id: string, body: Partial<RoutineBody>) =>
  api<Routine>(`/routines/${seg(id)}`, { key, method: 'PATCH', body });

/** Deletes a routine and its history. */
export const deleteRoutine = (key: string, id: string) => api(`/routines/${seg(id)}`, { key, method: 'DELETE' });

/** Clears a routine's finished runs; one still running stays. */
export const clearRuns = (key: string, id: string) =>
  api<Routine>(`/routines/${seg(id)}/runs`, { key, method: 'DELETE' });

/** Runs a cloud routine now; answers once the run is claimed. */
export const runNow = (key: string, id: string) =>
  api<Routine>(`/routines/${seg(id)}/run`, { key, method: 'POST', body: {} });
