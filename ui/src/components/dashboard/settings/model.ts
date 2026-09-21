/**
 * Pure rules for the AI model section: which provider a key is on, which
 * model is chosen, and what switching provider does to the draft.
 */
import { LLM_PRESETS, type KeyConfig } from '../config';
import { DEFAULT_LLM_PROVIDER, MODELS, PROVIDER_HOSTS } from './constants';

/** Unsaved edits, by config field. */
export type Draft = Record<string, string>;

/** The saved provider, or the one its base URL points at. */
export function savedProvider(config: KeyConfig | null): string {
  if (config?.llm_provider) return config.llm_provider;
  const baseUrl = config?.effective.baseUrl || '';
  return PROVIDER_HOSTS.find(([host]) => baseUrl.includes(host))?.[1] ?? DEFAULT_LLM_PROVIDER;
}

/** The preset models for a provider; none for an unknown one. */
export function presetModels(provider: string) {
  return Object.hasOwn(MODELS, provider) ? MODELS[provider] : [];
}

/**
 * Switching provider picks its default model and clears the base URL and key:
 * a key for one provider never works for another.
 */
export function switchProvider(draft: Draft, next: string): Draft {
  const chat_model = LLM_PRESETS.find((p) => p.id === next)!.model;
  const updated: Draft = { ...draft, llm_provider: next, chat_model, openai_base_url: '' };
  delete updated.openai_api_key;
  return updated;
}

/** Sets a field, or removes it from the draft when it is back to its saved value. */
export function withField(draft: Draft, field: string, next: string, saved: string): Draft {
  const updated = { ...draft };
  if (next === saved) delete updated[field];
  else updated[field] = next;
  return updated;
}

/** A string field of the saved config, or '' for anything else. */
export function savedField(config: KeyConfig | null, field: string): string {
  const value = config?.[field as keyof KeyConfig];
  return typeof value === 'string' ? value : '';
}
