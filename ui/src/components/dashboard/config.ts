/**
 * The key's settings: loading and saving them, the LLM presets the settings
 * dialog offers, and the one-click desktop sign-in link.
 */
import { apiOrigin } from '@/lib/api';
import { api } from '@/lib/api-client';

/** What GET /api/config returns. Settings belong to the API key, not an account. */
export interface KeyConfig {
  /** Which LLM preset the agent uses. */
  llm_provider: string;
  /** The LLM key, masked by the server. */
  openai_api_key: string;
  /** An OpenAI-compatible endpoint, when not the preset's own. */
  openai_base_url: string;
  /** The model the agent chats with. */
  chat_model: string;
  /** Where new browsers start by default. */
  browser_provider: string;
  /** Anchor credential. */
  anchor_api_key: string;
  /** Browserbase credential. */
  browserbase_api_key: string;
  /** Browserbase project. */
  browserbase_project_id: string;
  /** Steel credential. */
  steel_api_key: string;
  /** Browser Use credential. */
  browseruse_api_key: string;
  /** A bring-your-own CDP endpoint. */
  cdp_ws_url: string;
  /** Which CAPTCHA solver to use. */
  captcha_solver: string;
  /** The solver's credential. */
  captcha_api_key: string;
  /** "true" once the key has been through onboarding. */
  onboarded: string;
  /** When a desktop browser last connected with this key. */
  desktop_seen_at: string;
  /** Whether an LLM key is saved. */
  has_openai_key: boolean;
  /** Whether these settings come from a parent key. */
  inherited: boolean;
  /** What the agent will actually use once defaults are applied. */
  effective: EffectiveLlm;
  /** Every browser provider, what it needs, and whether it has it. */
  providers: ProviderStatus[];
}

/** The LLM the agent will use once defaults are applied. */
export interface EffectiveLlm {
  /** Endpoint. */
  baseUrl: string;
  /** Model. */
  model: string;
  /** Whether a key is available for it. */
  hasLlmKey: boolean;
}

/** A browser provider and whether this key can start browsers on it. */
export interface ProviderStatus {
  /** Provider id. */
  id: string;
  /** Display name. */
  label: string;
  /** Settings it needs. */
  needs: string[];
  /** Whether they are all set. */
  configured: boolean;
}

/** One LLM choice in the settings dialog. */
export interface LlmPreset {
  /** Provider id. */
  id: string;
  /** Display name. */
  label: string;
  /** Default model. */
  model: string;
  /** What its key looks like. */
  hint: string;
}

/** What POST /pairing returns. */
interface Pairing {
  /** Single-use code the desktop redeems. */
  code: string;
}

/** Reads the key's settings. */
export const loadConfig = (apiKey: string) => api<KeyConfig>('/config', { key: apiKey });

/** Saves some settings; the rest keep their values. Returns the result. */
export const saveConfig = (apiKey: string, values: Record<string, string>) =>
  api<KeyConfig>('/config', { key: apiKey, method: 'POST', body: values });

/** The LLMs the settings dialog offers, with a default model and a hint of the key's shape. */
export const LLM_PRESETS: LlmPreset[] = [
  { id: 'anthropic', label: 'Claude', model: 'claude-sonnet-5', hint: 'sk-ant-...' },
  { id: 'openai', label: 'OpenAI', model: 'gpt-4o-mini', hint: 'sk-...' },
  { id: 'gemini', label: 'Gemini', model: 'gemini-3.8-flash', hint: 'AIza...' },
  { id: 'vertex', label: 'Gemini Enterprise', model: 'gemini-2.5-flash', hint: 'AIza... (express mode)' },
];

/** Whether a provider runs on our own infrastructure, and so can reuse desktop cookies. */
export const isOyaProvider = (id: string) => id === 'oya-cloud' || id === 'oya-selfhosted';

/**
 * One-click desktop sign-in. The installed browser registers `oya://`, so this
 * hands it a single-use pairing code and the server to exchange it with.
 *
 * The key itself never goes in the URL: a protocol link is reachable by any page
 * the user visits, and it lands in OS logs on the way. The code expires in
 * minutes, redeems once, and the desktop app still asks before acting on it.
 */
export async function desktopSignInUrl(apiKey: string, profile = 'default'): Promise<string> {
  const { code } = await api<Pairing>('/pairing', { key: apiKey, method: 'POST', body: { profile } });
  const ws = `${apiOrigin().replace(/^http/, 'ws')}/ws`;
  return `oya://connect?code=${encodeURIComponent(code)}&server=${encodeURIComponent(ws)}`;
}
