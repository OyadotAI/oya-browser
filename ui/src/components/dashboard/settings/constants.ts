/**
 * Fixed choices in the settings dialog: its sections, the model presets per
 * provider, and the names of browser-provider credentials.
 */
import { Bell, Cpu, Monitor, ShieldCheck, Webhook } from 'lucide-react';

/** The dialog's sections, in tab order. */
export const SECTIONS = [
  { id: 'model' as const, label: 'AI model', icon: Cpu },
  { id: 'browsers' as const, label: 'Browsers', icon: Monitor },
  { id: 'verification' as const, label: 'Verification', icon: ShieldCheck },
  { id: 'alerts' as const, label: 'Alerts', icon: Bell },
  { id: 'webhooks' as const, label: 'Webhooks', icon: Webhook },
];

/** A settings section id. */
export type Section = (typeof SECTIONS)[number]['id'];

/** One preset model in the picker. */
export interface ModelPreset {
  /** The id the provider's API expects. */
  id: string;
  /** What the picker shows. */
  label: string;
}

/** Preset models offered for each LLM provider; anything else is a custom model id. */
export const MODELS: Record<string, ModelPreset[]> = {
  openai: [
    { id: 'gpt-4o-mini', label: 'GPT-4o mini' },
    { id: 'gpt-4.1-mini', label: 'GPT-4.1 mini' },
    { id: 'gpt-4.1', label: 'GPT-4.1' },
  ],
  anthropic: [
    { id: 'claude-opus-5', label: 'Claude Opus 5' },
    { id: 'claude-sonnet-5', label: 'Claude Sonnet 5' },
    { id: 'claude-haiku-4-5', label: 'Claude Haiku 4.5' },
  ],
  gemini: [
    { id: 'gemini-3.8-flash', label: 'Gemini 3.8 Flash' },
    { id: 'gemini-3.5-flash-lite', label: 'Gemini 3.5 Flash-Lite' },
    { id: 'gemini-3.1-pro-preview', label: 'Gemini 3.1 Pro (preview)' },
  ],
  vertex: [
    { id: 'gemini-2.5-flash', label: 'Gemini 2.5 Flash' },
    { id: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro' },
  ],
};

/**
 * With no provider saved, the base URL says which one the key talks to. Checked
 * in order; anything else is OpenAI-compatible.
 */
export const PROVIDER_HOSTS: [host: string, provider: string][] = [
  ['anthropic.com', 'anthropic'],
  ['aiplatform.googleapis.com', 'vertex'],
  ['generativelanguage.googleapis.com', 'gemini'],
];

/** The provider assumed when nothing else says otherwise. */
export const DEFAULT_LLM_PROVIDER = 'openai';

/** The model select's "Custom model…" option value. */
export const CUSTOM_MODEL = '__custom';

/** Human labels for browser-provider credential fields; others show the field name. */
export const CREDENTIAL_LABELS: Record<string, string> = {
  anchor_api_key: 'Anchor API key',
  browserbase_api_key: 'Browserbase API key',
  browserbase_project_id: 'Project ID',
  steel_api_key: 'Steel API key',
  browseruse_api_key: 'Browser Use API key',
  cdp_ws_url: 'Chrome connection URL',
};

/** Placeholder for a secret that is already stored. */
export const SAVED_PLACEHOLDER = 'Saved · leave blank to keep';

/** Slack setup steps, numbered as shown. */
export const SlackStep = {
  /** Connect the workspace. */
  CONNECT: 1,
  /** Choose the alert channel. */
  CHANNEL: 2,
} as const;
