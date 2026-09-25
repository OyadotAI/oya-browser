/**
 * The LLM providers a key can use and the models each one offers. The server
 * is the one place these live: the console, the desktop app and the CLI all
 * read them from GET /config, so the lists cannot disagree and a model picked
 * in one client shows as picked in the others.
 */
import { openRouterModels } from './openrouter-models.ts';

/** One model a picker offers. */
export interface ModelChoice {
  /** The id the provider's API expects. */
  id: string;
  /** What the picker shows. */
  label: string;
}

/** A shorthand for a list of model choices: [id, label] pairs. */
const models = (pairs: [string, string][]): ModelChoice[] => pairs.map(([id, label]) => ({ id, label }));

/** Gemini's models, offered by both Google endpoints. */
const GEMINI = models([
  ['gemini-3.8-flash', 'Gemini 3.8 Flash'],
  ['gemini-3.7-flash', 'Gemini 3.7 Flash'],
  ['gemini-3.6-flash', 'Gemini 3.6 Flash'],
  ['gemini-3.5-flash-lite', 'Gemini 3.5 Flash-Lite'],
  ['gemini-3.1-pro-preview', 'Gemini 3.1 Pro (preview)'],
]);

/**
 * Each provider's endpoint, default model, how its key looks and where to get
 * one, and the models its picker offers (anything else is a custom model id).
 * Defaults are what a key with no chat_model runs on: changing one changes
 * what existing keys use.
 */
export const LLM_DEFAULTS = {
  openai: {
    label: 'OpenAI',
    base: 'https://api.openai.com/v1',
    model: 'gpt-4o-mini',
    hint: 'sk-...',
    keysUrl: 'https://platform.openai.com/api-keys',
    models: models([
      ['gpt-6-sol', 'GPT-6 Sol'],
      ['gpt-6-luna', 'GPT-6 Luna'],
      ['gpt-6-astra', 'GPT-6 Astra'],
      ['gpt-4.1', 'GPT-4.1'],
      ['gpt-4.1-mini', 'GPT-4.1 mini'],
      ['gpt-4o-mini', 'GPT-4o mini'],
    ]),
  },
  // Claude's own Messages API (platform/llm/anthropic.ts): thinking, caching and parallel tool calls.
  // Its ids use dashes (claude-opus-5-5); OpenRouter's use dots (anthropic/claude-opus-5.5).
  anthropic: {
    label: 'Claude',
    base: 'https://api.anthropic.com/v1',
    model: 'claude-opus-5',
    hint: 'sk-ant-...',
    keysUrl: 'https://console.anthropic.com/settings/keys',
    models: models([
      ['claude-opus-5-5', 'Claude Opus 5.5'],
      ['claude-fable-5-1', 'Claude Fable 5.1'],
      ['claude-opus-5', 'Claude Opus 5'],
      ['claude-sonnet-5', 'Claude Sonnet 5'],
      ['claude-haiku-4-5', 'Claude Haiku 4.5'],
    ]),
  },
  gemini: {
    label: 'Gemini',
    base: 'https://generativelanguage.googleapis.com/v1beta/openai',
    model: 'gemini-3.8-flash',
    hint: 'AIza...',
    keysUrl: 'https://aistudio.google.com/apikey',
    models: GEMINI,
  },
  // Gemini Enterprise (ex-Vertex AI) in express mode: a global endpoint with no project
  // or location, and an API key that only authenticates against native generateContent,
  // the OpenAI-compatible .../endpoints/openapi path wants an OAuth token instead. llm.js
  // translates for this base URL. A project-scoped enterprise endpoint still works: set
  // openai_base_url to .../endpoints/openapi and use an access token as the key.
  vertex: {
    label: 'Gemini Enterprise',
    base: 'https://aiplatform.googleapis.com/v1/publishers/google',
    model: 'gemini-2.5-flash',
    hint: 'AIza... (express mode)',
    keysUrl: 'https://console.cloud.google.com/vertex-ai/studio/settings/api-keys',
    models: [
      ...GEMINI,
      ...models([
        ['gemini-2.5-flash', 'Gemini 2.5 Flash'],
        ['gemini-2.5-pro', 'Gemini 2.5 Pro'],
      ]),
    ],
  },
  // OpenAI-compatible, so it runs through platform/llm/openai.ts. Its models are read live
  // from OpenRouter (openrouter-models.ts); this list is only what a picker shows until then.
  openrouter: {
    label: 'OpenRouter',
    base: 'https://openrouter.ai/api/v1',
    model: 'anthropic/claude-sonnet-5',
    hint: 'sk-or-...',
    keysUrl: 'https://openrouter.ai/keys',
    models: models([
      ['anthropic/claude-sonnet-5', 'Anthropic: Claude Sonnet 5'],
      ['anthropic/claude-opus-5.5', 'Anthropic: Claude Opus 5.5'],
      ['openai/gpt-6-sol', 'OpenAI: GPT-6 Sol'],
      ['openai/gpt-6-luna', 'OpenAI: GPT-6 Luna'],
      ['google/gemini-3.8-flash', 'Google: Gemini 3.8 Flash'],
      ['x-ai/grok-4.7', 'xAI: Grok 4.7'],
      ['deepseek/deepseek-v4-pro', 'DeepSeek: DeepSeek V4 Pro'],
      ['moonshotai/kimi-k3', 'MoonshotAI: Kimi K3'],
      ['qwen/qwen3.8-max-prime', 'Qwen: Qwen3.8 Max'],
      ['z-ai/glm-5.3', 'Z.ai: GLM 5.3'],
      ['mistralai/mistral-medium-3-5', 'Mistral: Mistral Medium 3.5'],
      ['meta-llama/llama-4-maverick', 'Meta: Llama 4 Maverick'],
    ]),
  },
};

/** One provider as a picker needs it. */
export interface CatalogEntry {
  /** The llm_provider value. */
  id: string;
  /** Its name. */
  label: string;
  /** How its key looks, for a placeholder. */
  hint: string;
  /** Where to get a key. */
  keysUrl: string;
  /** Its endpoint, so a client can tell which provider a key's effective base URL is. */
  base: string;
  /** The model a key runs on when it names none. */
  model: string;
  /** The models to offer. */
  models: ModelChoice[];
}

/** The model list a provider's picker shows: OpenRouter's live one, the built-in list for the rest. */
const modelsFor = (id: string, models: ModelChoice[]) => (id === 'openrouter' ? openRouterModels(models) : models);

/** Every provider with its models, as GET /config returns it under llm_catalog. */
export const llmCatalog = (): CatalogEntry[] =>
  Object.entries(LLM_DEFAULTS).map(([id, { label, hint, keysUrl, base, model, models }]) => ({
    ...{ id, label, hint, keysUrl, base, model },
    models: modelsFor(id, models),
  }));
