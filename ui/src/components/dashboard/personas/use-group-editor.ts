/**
 * The group editor's state: its name (a new group only), the ticked profiles,
 * and the save, whose refusal stays in the editor in the server's words.
 */
import { useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { createGroup, replaceGroup, type PersonaGroup } from './group-api';

/** The fields, a toggle, and save, which hands the saved group to `onSaved`. */
export function useGroupEditor(apiKey: string, group: PersonaGroup | null, onSaved: () => void) {
  const [name, setName] = useState(group?.name ?? '');
  const [picked, setPicked] = useState<string[]>(group?.personaIds ?? []);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const toggle = (id: string) => setPicked((ids) => (ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id]));
  const send = () => (group ? replaceGroup(apiKey, group.name, picked) : createGroup(apiKey, name.trim(), picked));
  const save = () => submit({ send, onSaved, setSaving, setError });
  return { name, setName, picked, toggle, saving, error, save, canSave: picked.length > 0 && (group || name.trim()) };
}

/** What a save needs. */
interface Submit {
  /** Sends the request. */
  send: () => Promise<unknown>;
  /** Runs after it was saved. */
  onSaved: () => void;
  /** Marks the save in flight. */
  setSaving: (saving: boolean) => void;
  /** Shows the server's refusal. */
  setError: (error: string | null) => void;
}

/** Saves, keeping the server's refusal in the editor. */
async function submit({ send, onSaved, setSaving, setError }: Submit) {
  setSaving(true);
  setError(null);
  await send().then(onSaved, (err) => setError(errorMessage(err)));
  setSaving(false);
}

/** What the hook gives the editor. */
export type GroupEditor = ReturnType<typeof useGroupEditor>;
