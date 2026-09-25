/**
 * The editor's unsaved changes. A field set back to its saved value leaves the
 * draft, so "dirty" means something really changed.
 */
import { useState } from 'react';
import type { KeyConfig } from '../config';
import { savedField, withField, type Draft } from './model';

/** Reads and edits the settings form over the saved `config`. */
export function useDraft(config: KeyConfig | null) {
  const [draft, setDraft] = useState<Draft>({});
  const saved = (field: string) => savedField(config, field);
  const value = (field: string) => draft[field] ?? saved(field);
  const set = (field: string, next: string) => setDraft((previous) => withField(previous, field, next, saved(field)));
  return { config, draft, setDraft, saved, value, set, dirty: Object.keys(draft).length > 0 };
}

/** The form as sections see it. */
export type SettingsForm = ReturnType<typeof useDraft>;
