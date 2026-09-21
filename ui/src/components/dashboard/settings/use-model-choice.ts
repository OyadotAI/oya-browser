/**
 * The AI model section's state: which provider and model are chosen, whether
 * a custom model id is being typed, and whether a switch still needs a key.
 */
import { useState } from 'react';
import type { KeyConfig } from '../config';
import { CUSTOM_MODEL } from './constants';
import { presetModels, savedProvider, switchProvider } from './model';
import type { SettingsForm } from './use-draft';

/** Provider and model as the form currently has them. */
function chosen(config: KeyConfig | null, form: SettingsForm) {
  const original = savedProvider(config);
  const provider = form.value('llm_provider') || original;
  const model = form.value('chat_model') || config?.effective.model || '';
  const providerChanged = provider !== original;
  return { provider, model, providerChanged, needsModelKey: providerChanged && !form.draft.openai_api_key?.trim() };
}

/** Choosing another provider resets the model to its default and leaves custom-id mode. */
function chooseProvider(next: string, current: string, form: SettingsForm, setCustom: (on: boolean) => void) {
  if (current === next) return;
  setCustom(false);
  form.setDraft((draft) => switchProvider(draft, next));
}

/** "Custom model…" opens the id field; a preset is stored as the model. */
function pickModel(next: string, form: SettingsForm, setCustom: (on: boolean) => void) {
  setCustom(next === CUSTOM_MODEL);
  if (next !== CUSTOM_MODEL) form.set('chat_model', next);
}

/** The model choice, plus `chooseProvider` and `pickModel` for the controls. */
export function useModelChoice(config: KeyConfig | null, form: SettingsForm) {
  const [customModel, setCustomModel] = useState(false);
  const state = chosen(config, form);
  const models = presetModels(state.provider);
  const custom = customModel || !models.some((m) => m.id === state.model);
  const onProvider = (next: string) => chooseProvider(next, state.provider, form, setCustomModel);
  const onModel = (next: string) => pickModel(next, form, setCustomModel);
  return { ...state, models, custom, chooseProvider: onProvider, pickModel: onModel };
}

/** The model section's state and controls. */
export type ModelChoice = ReturnType<typeof useModelChoice>;
