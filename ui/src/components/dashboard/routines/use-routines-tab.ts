/**
 * The Routines tab's state in one place: the list, the rows' actions, and which
 * routine the editor is open on ('new' for a new one).
 */
import { useState } from 'react';
import { useRoutineActions } from './use-routine-actions';
import { useRoutineList } from './use-routine-list';
import type { Routine } from './types';

/** The list, the actions, and the editor's routine with ways to open, close and finish it. */
export function useRoutinesTab(apiKey: string) {
  const list = useRoutineList(apiKey);
  const actions = useRoutineActions(apiKey, list);
  const [editing, setEditing] = useState<Routine | 'new' | null>(null);
  const saved = (routine: Routine) => (list.replace(routine), setEditing(null));
  return { list, actions, editing, setEditing, saved };
}
