/**
 * The settings editor's state and actions: load, edit, save, and open the
 * desktop app for sign-in.
 */
import { useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import { desktopSignInUrl, saveConfig } from '../config';
import { withBusy } from './busy';
import { useDraft, type SettingsForm } from './use-draft';
import { useKeyConfig } from './use-key-config';
import { useModelChoice, type ModelChoice } from './use-model-choice';

/** Saving is refused until something changed, a switched provider has a key, and a model is named. */
function blocked(form: SettingsForm, choice: ModelChoice) {
  return !form.dirty || choice.needsModelKey || !choice.model.trim();
}

/** Opens the desktop app through a one-click sign-in link; a failure shows in the dialog. */
function useOpenDesktop(apiKey: string, setError: (e: string) => void) {
  const [pairing, setPairing] = useState(false);
  const open = async () => void (window.location.href = await desktopSignInUrl(apiKey));
  const onError = (err: unknown) => setError(errorMessage(err, 'Could not open desktop.'));
  return { pairing, openDesktop: () => withBusy(setPairing, open, onError) };
}

/** Saves the draft, then confirms and closes; a failure stays in the dialog. */
function useSave(apiKey: string, draft: Record<string, string>, setError: (e: string) => void, onClose: () => void) {
  const toast = useToast();
  const [saving, setSaving] = useState(false);
  const store = async () => (await saveConfig(apiKey, draft), toast('Settings saved', 'success'), onClose());
  const onError = (err: unknown) => setError(errorMessage(err, 'Could not save settings.'));
  return { saving, save: () => (setError(''), withBusy(setSaving, store, onError)) };
}

/** Everything the editor renders from. `close` is refused while saving. */
export function useSettingsEditor(apiKey: string, onClose: () => void) {
  const load = useKeyConfig(apiKey);
  const form = useDraft(load.config);
  const choice = useModelChoice(load.config, form);
  const { saving, save } = useSave(apiKey, form.draft, load.setError, onClose);
  const canSave = !!load.config && !blocked(form, choice) && !saving;
  const guardedSave = () => void (!blocked(form, choice) && !saving && save());
  const close = () => void (!saving && onClose());
  return { ...load, ...useOpenDesktop(apiKey, load.setError), form, choice, saving, canSave, save: guardedSave, close };
}

/** The editor as its parts see it. */
export type SettingsEditorState = ReturnType<typeof useSettingsEditor>;
