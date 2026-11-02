/**
 * The create and edit form: its fields, built from a routine or a new one's
 * defaults; what is wrong with them, shown only after the first save attempt;
 * and the body a save sends, with the person's own time zone.
 */
import { useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { createRoutine, editRoutine } from './api';
import { usePlaybookNames } from './use-playbook-names';
import { DEFAULT_SCHEDULE, EDITOR_TEXT, EVERY_MAX } from './constants';
import type { PlaybookBody } from '../playbooks/types';
import type { Routine, RoutineBody } from './types';

/** The form's fields. */
export interface RoutineFields {
  /** The routine's name. */
  name: string;
  /** The playbook it plays, or '' to ask the prompt. */
  playbook: string;
  /** What it asks the agent. */
  prompt: string;
  /** every or daily. */
  kind: 'every' | 'daily';
  /** How many units apart, as typed. */
  n: string;
  /** minutes or hours. */
  unit: string;
  /** HH:MM for daily. */
  at: string;
  /** Runs in the cloud. */
  cloud: boolean;
  /** Runs on its schedule. */
  enabled: boolean;
  /** What it sets the playbook's variables to; a blank one uses the playbook's own. */
  values: Record<string, string>;
  /** The persona it runs as, or '' for the default. */
  persona: string;
}

/** What is wrong with the fields, by field. */
export interface FieldErrors {
  /** The name. */
  name?: string;
  /** The prompt. */
  prompt?: string;
  /** The every count. */
  every?: string;
}

/** The person's own time zone, which a daily time is read in. */
export const ownZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone;

/** The schedule's fields from a routine, or a new one's. */
function scheduleFields(routine: Routine | null) {
  const s = routine?.schedule;
  const n = String(s?.n ?? DEFAULT_SCHEDULE.n);
  return { kind: s?.kind ?? 'every', n, unit: s?.unit ?? DEFAULT_SCHEDULE.unit, at: s?.at ?? DEFAULT_SCHEDULE.at };
}

/** Where and as whom it runs, and whether it is on, from a routine or a new one's. */
const placeFields = (routine: Routine | null) => ({
  cloud: routine?.target === 'cloud',
  enabled: routine?.enabled ?? true,
  persona: routine?.persona ?? '',
});

/** The fields for editing `routine`, or a new routine's when it is null. */
export function fieldsOf(routine: Routine | null): RoutineFields {
  return {
    name: routine?.name ?? '',
    playbook: routine?.playbook ?? '',
    prompt: routine?.prompt ?? '',
    ...(scheduleFields(routine) as Pick<RoutineFields, 'kind' | 'n' | 'unit' | 'at'>),
    ...placeFields(routine),
    values: { ...routine?.values },
  };
}

/**
 * The values to save: only those `playbook` uses, not its secrets, and not blank
 * (a blank one plays the playbook's own). With the list not loaded, what the
 * routine had is kept.
 */
export function valuesFor(playbook: PlaybookBody | undefined, values: Record<string, string>) {
  const kept = Object.entries(values).filter(([k, v]) => v !== '' && !playbook?.secrets?.includes(k));
  return Object.fromEntries(playbook ? kept.filter(([k]) => playbook.variables.includes(k)) : kept);
}

/** Whether an every count is a whole number in range. */
const goodEvery = (n: string) => /^\d+$/.test(n) && Number(n) >= 1 && Number(n) <= EVERY_MAX;

/** What is wrong with `f`; empty when it can be saved. */
export function errorsOf(f: RoutineFields): FieldErrors {
  const errors: FieldErrors = {};
  if (!f.name.trim()) errors.name = EDITOR_TEXT.needName;
  if (!f.playbook && !f.prompt.trim()) errors.prompt = EDITOR_TEXT.needPrompt;
  if (f.kind === 'every' && !goodEvery(f.n)) errors.every = EDITOR_TEXT.badEvery;
  return errors;
}

/** The schedule a save sends. */
const scheduleOf = (f: RoutineFields) =>
  f.kind === 'daily' ? { kind: f.kind, at: f.at } : { kind: f.kind, n: Number(f.n), unit: f.unit };

/** What saving `f` sends; a playbook replaces the prompt (with its values), null asks the prompt again. */
export function bodyOf(f: RoutineFields, chosen?: PlaybookBody): RoutineBody {
  const playbook = f.playbook || null;
  const target = f.cloud ? 'cloud' : 'desktop';
  const task = playbook ? { prompt: '', values: valuesFor(chosen, f.values) } : { prompt: f.prompt.trim() };
  const where = { target, tz: ownZone(), persona: f.persona || null } as const;
  return { name: f.name.trim(), playbook, ...task, schedule: scheduleOf(f), enabled: f.enabled, ...where };
}

/** Sends the save: a new routine, or the edit of `routine`. */
const send = (apiKey: string, routine: Routine | null, body: RoutineBody) =>
  routine ? editRoutine(apiKey, routine.id, body) : createRoutine(apiKey, body);

/** Whether a save is in flight, and the server's last refusal. */
function useSaveState() {
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  return { saving, setSaving, saveError, setSaveError };
}

/** The form's state, its errors, and save, which hands the saved routine to `onSaved`. */
export function useRoutineForm(apiKey: string, routine: Routine | null, onSaved: (r: Routine) => void) {
  const playbooks = usePlaybookNames(apiKey);
  const [fields, setFields] = useState(() => fieldsOf(routine));
  const [tried, setTried] = useState(false);
  const { setSaving, setSaveError, ...saveState } = useSaveState();
  const set = (change: Partial<RoutineFields>) => setFields((f) => ({ ...f, ...change }));
  const chosen = playbooks.list.find((p) => p.name === fields.playbook);
  const save = () => submit({ apiKey, routine, fields, chosen, onSaved, setTried, setSaving, setSaveError });
  return { fields, set, errors: tried ? errorsOf(fields) : {}, ...saveState, save, playbooks, chosen };
}

/** What a save needs from the form. */
interface Submit {
  /** The caller's API key. */
  apiKey: string;
  /** The routine being edited, or null for a new one. */
  routine: Routine | null;
  /** The fields to save. */
  fields: RoutineFields;
  /** The playbook chosen, as listed, to keep only the values it uses. */
  chosen?: PlaybookBody;
  /** Receives the saved routine. */
  onSaved: (r: Routine) => void;
  /** Marks that a save was tried, so errors show. */
  setTried: (tried: boolean) => void;
  /** Marks the save in flight. */
  setSaving: (saving: boolean) => void;
  /** Shows the server's refusal. */
  setSaveError: (error: string | null) => void;
}

/** Saves the fields when they are good; the server's refusal stays in the form. */
async function submit(s: Submit) {
  s.setTried(true);
  if (Object.keys(errorsOf(s.fields)).length) return;
  s.setSaving(true);
  s.setSaveError(null);
  await sendSave(s);
  s.setSaving(false);
}

/** Sends the save and hands on the saved routine, or keeps the refusal. */
async function sendSave(s: Submit) {
  try {
    s.onSaved(await send(s.apiKey, s.routine, bodyOf(s.fields, s.chosen)));
  } catch (err) {
    s.setSaveError(errorMessage(err));
  }
}

/** What the form hook gives the dialog. */
export type RoutineForm = ReturnType<typeof useRoutineForm>;
