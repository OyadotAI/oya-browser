/** Shared model configuration rules keep onboarding and Ask on the same provider contract. */
import type { Payload } from '../../shared/ipc.ts';

/** One model provider the server offers. */
interface Provider {
  /** Its id ('openai', 'anthropic'...). */
  id: string;
  /** Its name for people. */
  label?: string;
  /** Its API endpoint. */
  base?: string;
  /** Its default model. */
  model?: string;
  /** Its models. */
  models?: unknown[];
}

/** The model settings in effect. */
interface Effective {
  /** Whether the project has a model key. */
  hasLlmKey?: boolean;
  /** The model in use. */
  model?: string;
  /** The endpoint in use. */
  baseUrl?: string;
}

/** Preserve an existing endpoint unless a provider change or explicit edit overrides it. */
function modelEndpoint(changed: boolean, value: unknown): Payload {
  return typeof value === 'string' ? endpointUpdate(value) : changed ? { openai_base_url: null } : {};
}

/** Validates explicit custom endpoints before saving project model settings. */
function endpointUpdate(value: string): Payload {
  if (!value.trim()) return { openai_base_url: null };
  try {
    const url = new URL(value.trim());
    if (!['https:', 'http:'].includes(url.protocol) || url.username || url.password) throw new Error();
    return { openai_base_url: value.trim() };
  } catch {
    return { error: 'Enter an HTTP or HTTPS model endpoint without embedded credentials.' };
  }
}

/** The project's model settings, as GET /config answers them. */
export interface ServerConfig {
  /** The provider the key saved. */
  llm_provider?: string;
  /** The providers the server offers. */
  llm_catalog?: Provider[];
  /** What is in effect: whether there is a key, the model and the endpoint. */
  effective?: Effective;
}

/** A choice made in Ask: provider, model, and a key when one was typed. */
export interface ModelChoice {
  /** The provider's id. */
  provider?: unknown;
  /** The model, or nothing for the provider's default. */
  model?: unknown;
  /** The key, when typed. */
  key?: unknown;
  /** Optional custom API endpoint; empty restores the provider default. */
  baseUrl?: unknown;
}

/**
 * What Ask offers against a server too old to send its catalog: the providers
 * it has always taken, each on its default model (any other is typed by hand).
 */
const LEGACY_CATALOG: Provider[] = [
  ['anthropic', 'Claude', 'sk-ant-...', 'https://console.anthropic.com/settings/keys', 'claude-opus-5'],
  ['openai', 'OpenAI', 'sk-...', 'https://platform.openai.com/api-keys', 'gpt-4o-mini'],
  ['gemini', 'Gemini', 'AIza...', 'https://aistudio.google.com/apikey', 'gemini-3.8-flash'],
].map(([id, label, hint, keysUrl, model]) => ({
  id,
  label,
  hint,
  keysUrl,
  model,
  models: [{ id: model, label: model }],
}));

/** The server's provider catalog, or the legacy one from a server that has none. */
const catalogOf = (config: ServerConfig): Provider[] =>
  config.llm_catalog?.length ? config.llm_catalog : LEGACY_CATALOG;

/** The provider a key runs on: the one it saved, else the catalog entry whose endpoint it is using. */
function currentProvider(config: ServerConfig): string {
  if (config.llm_provider) return config.llm_provider;
  return catalogOf(config).find((p) => p.base && p.base === config.effective?.baseUrl)?.id || '';
}

/** What the model card needs from the server's config. */
export function statusOf(config: ServerConfig, requireCatalog = false): Payload {
  return requireCatalog && !config.llm_catalog?.length ? CATALOG_UNAVAILABLE : modelStatus(config);
}
/** A real catalog is mandatory for new setup; legacy Ask retains its established fallback. */
const CATALOG_UNAVAILABLE = { error: 'This server needs an update to support model setup.' };
/** The existing Ask status remains compatible with servers that predate catalogs. */
function modelStatus(config: ServerConfig): Payload {
  const { hasLlmKey, model } = config.effective || {};
  return {
    hasLlmKey: !!hasLlmKey,
    provider: currentProvider(config),
    model,
    baseUrl: config.effective?.baseUrl || '',
    catalog: catalogOf(config),
  };
}

/**
 * The POST /config body for a choice made in Ask, or `{ error }`. The model is
 * always sent (never nulled behind the person's back), a key only when one was
 * typed, and the endpoint is reset only when the provider changes, so saving
 * here never undoes a model or gateway chosen in the console.
 */
export function modelUpdate(config: ServerConfig, choice: ModelChoice = {}, requireCatalog = false): Payload {
  return requireCatalog && !config.llm_catalog?.length ? CATALOG_UNAVAILABLE : modelBody(config, choice);
}
/** Common provider and endpoint validation is shared by Ask and Technical setup. */
function modelBody(config: ServerConfig, { provider, model, key, baseUrl }: ModelChoice): Payload {
  const secret = typeof key === 'string' ? key.trim() : '';
  if (!catalogOf(config).some((p) => p.id === provider)) return { error: 'Pick a provider.' };
  const changed = provider !== currentProvider(config);
  if (!secret && (changed || !config.effective?.hasLlmKey)) return { error: 'Paste your API key for this provider.' };
  const body: Payload = { llm_provider: provider, chat_model: (typeof model === 'string' && model.trim()) || null };
  if (secret) body.openai_api_key = secret;
  Object.assign(body, modelEndpoint(changed, baseUrl));
  return body;
}
