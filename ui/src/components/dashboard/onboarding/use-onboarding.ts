/**
 * Onboarding's state: whether the desktop has connected, the AI model chosen
 * for Ask and its key, the step the preview shows, and the pair and finish actions.
 */
import { useState } from 'react';
import { LLM_PRESETS, saveConfig, type KeyConfig } from '../config';
import { useDesktopSignIn } from '../hooks/use-desktop-sign-in';
import { useBusyAction } from '../hooks/use-busy-action';
import type { BrowserRow, Persona } from '../types';

/** What onboarding is given. */
export interface OnboardingInput {
  /** The key being set up. */
  apiKey: string;
  /** Its settings. */
  config: KeyConfig;
  /** Its personas. */
  personas: Persona[];
  /** Its browsers, to see whether a desktop has connected. */
  browsers: BrowserRow[];
  /** Setup is saved: open the console. */
  onDone: () => void;
}

/** Everything the onboarding page shows and does. */
export function useOnboarding(input: OnboardingInput) {
  const model = useModelForm(input.config);
  const pairing = useDesktopSignIn(input.apiKey);
  const saving = useFinish(input, model.provider, model.key);
  const busy = pairing.busy ? 'pair' : saving.busy ? 'save' : null;
  const hasModel = !!input.config.effective?.hasLlmKey;
  const desktop = desktopOf(input);
  const preview = usePreviewStep(!!desktop);
  return { ...model, ...preview, desktop, hasModel, busy, pair: () => pairing.open(), finish: saving.finish };
}

/** A step of onboarding, by name. */
export type Step = 'desktop' | 'model';

/** The step the preview shows: the one the person points at, else the first not done. */
function usePreviewStep(desktopDone: boolean) {
  const [focused, setFocused] = useState<Step | null>(null);
  const step: Step = focused ?? (desktopDone ? 'model' : 'desktop');
  return { step, setFocused };
}

/** The AI provider Ask runs on and the key typed for it. */
function useModelForm(config: KeyConfig) {
  const known = LLM_PRESETS.some((p) => p.id === config.llm_provider);
  const [provider, setProvider] = useState(known ? config.llm_provider : LLM_PRESETS[0].id);
  const [key, setKey] = useState('');
  return { provider, setProvider, key, setKey };
}

/** The desktop running as the default profile, once one has connected. */
function desktopOf({ personas, browsers }: OnboardingInput) {
  const profile = personas.find((p) => p.isDefault);
  return browsers.find((b) => b.provider === 'oya-desktop' && b.persona === profile?.id);
}

/** A typed key as settings, on the provider's own defaults; nothing when none was typed. */
const modelSettings = (provider: string, key: string): Record<string, string> =>
  key.trim() ? { llm_provider: provider, openai_api_key: key.trim(), chat_model: '', openai_base_url: '' } : {};

/** Saves the model key (when one was typed), marks the key onboarded, then calls `onDone`. */
function useFinish({ apiKey, onDone }: OnboardingInput, provider: string, key: string) {
  const { busy, run } = useBusyAction();
  const finish = () =>
    run(async () => {
      await saveConfig(apiKey, { ...modelSettings(provider, key), onboarded: 'true' });
      onDone();
    });
  return { busy, finish };
}
