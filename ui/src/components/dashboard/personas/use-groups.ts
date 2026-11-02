/**
 * The Groups dialog's state: the key's groups (null while loading), why they
 * did not load, which group the editor is open on, and the delete waiting to be
 * confirmed. Loads when the dialog opens.
 */
import { useCallback, useEffect, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import { deleteGroup, listGroups, type PersonaGroup } from './group-api';

/** The editor's subject: a group, a new one, or none. */
export type Editing = PersonaGroup | 'new' | null;

/** The list, a reload, and the editor and delete state. */
export function useGroups(apiKey: string, open: boolean) {
  const list = useList(apiKey, open);
  return { ...list, ...useSubjects(), ...useRemove(apiKey, list.load) };
}

/** Loads the groups into `setGroups`, or marks the load failed. */
function fetchGroups(apiKey: string, setGroups: (g: PersonaGroup[]) => void, setFailed: (f: boolean) => void) {
  const loaded = (list: PersonaGroup[]) => (setFailed(false), setGroups(list));
  return listGroups(apiKey).then(loaded, () => setFailed(true));
}

/** The groups (null while loading), whether the last load failed, and a reload; loads when `open`. */
function useList(apiKey: string, open: boolean) {
  const [groups, setGroups] = useState<PersonaGroup[] | null>(null);
  const [failed, setFailed] = useState(false);
  const load = useCallback(() => void fetchGroups(apiKey, setGroups, setFailed), [apiKey]);
  useEffect(() => void (open && load()), [open, load]);
  return { groups, failed, load };
}

/** Which group the editor is open on, and which delete waits to be confirmed. */
function useSubjects() {
  const [editing, setEditing] = useState<Editing>(null);
  const [confirming, setConfirming] = useState<PersonaGroup | null>(null);
  return { editing, setEditing, confirming, setConfirming };
}

/** Deleting a group: in flight while `deleting`, a failure toasted with the server's reason. */
function useRemove(apiKey: string, load: () => void) {
  const [deleting, setDeleting] = useState(false);
  const toast = useToast();
  const remove = async (name: string) => {
    setDeleting(true);
    await deleteGroup(apiKey, name).then(load, (err) => toast(errorMessage(err), 'error'));
    setDeleting(false);
  };
  return { deleting, remove };
}

/** What the hook gives the dialog. */
export type GroupsState = ReturnType<typeof useGroups>;
