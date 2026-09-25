/**
 * Fixed choices in the settings dialog: its sections, how a provider is told
 * from its endpoint, and the names of browser-provider credentials.
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

// The model presets come from the server's catalog (llmCatalog in ../config).
export type { ModelPreset } from '../config';

/**
 * With no provider saved, the base URL says which one the key talks to. Checked
 * in order; anything else is OpenAI-compatible.
 */
export const PROVIDER_HOSTS: [host: string, provider: string][] = [
  ['anthropic.com', 'anthropic'],
  ['aiplatform.googleapis.com', 'vertex'],
  ['generativelanguage.googleapis.com', 'gemini'],
  ['openrouter.ai', 'openrouter'],
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
