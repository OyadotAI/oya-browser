/**
 * Types for a key's settings: the LLM behind `ask()`, the browser provider and
 * the CAPTCHA solver, as `config.set()` accepts them and `config.get()` returns them.
 */
import type { Provider } from './browsers.js';

/** Which model drives `ask()` and the chat API. */
export type LlmProvider =
  | 'openai'
  | 'anthropic'
  /** Gemini via AI Studio. */
  | 'gemini'
  /** Gemini Enterprise, ex-Vertex AI. Express mode by default; set `openai_base_url` to a
   *  project-scoped `.../endpoints/openapi` endpoint to use an enterprise project. */
  | 'vertex';

/** Empty disables solving. */
export type CaptchaSolver = 'capsolver' | '2captcha' | '';

/**
 * What a key can configure. Mirrors the server's field allowlist exactly: a field not
 * listed here is ignored rather than stored, so the type is the whole surface.
 * `null` clears a field and falls back to the deployment default.
 */
export interface ConfigUpdate {
  /** Which LLM vendor `ask()` and the chat API call. */
  llm_provider?: LlmProvider | null;
  /** The credential for whichever `llm_provider` is set, the field name is shared. */
  openai_api_key?: string | null;
  /** Only honoured alongside this key's own `openai_api_key`. */
  openai_base_url?: string | null;
  /** Overrides the provider's default model. */
  chat_model?: string | null;
  /** Where this key's browsers run unless `start()` names a provider. */
  browser_provider?: Provider | null;
  /** Anchor's API key, for the 'anchor' provider. */
  anchor_api_key?: string | null;
  /** Browserbase's API key, for the 'browserbase' provider. */
  browserbase_api_key?: string | null;
  /** The Browserbase project browsers are created in. */
  browserbase_project_id?: string | null;
  /** Steel's API key, for the 'steel' provider. */
  steel_api_key?: string | null;
  /** Browser Use Cloud's API key, for the 'browseruse' provider. */
  browseruse_api_key?: string | null;
  /** The DevTools WebSocket URL of your own Chrome, for the 'cdp' provider. */
  cdp_ws_url?: string | null;
  /** Which CAPTCHA solving service to call. */
  captcha_solver?: CaptchaSolver | null;
  /** The solving service's API key. */
  captcha_api_key?: string | null;
  /** Set once onboarding has been completed, so the console stops offering it. */
  onboarded?: string | null;
}

/** What `config.get()` returns. Secrets read back masked, never in full. */
export interface Config extends Omit<ConfigUpdate, 'llm_provider' | 'browser_provider' | 'captcha_solver'> {
  /** The configured LLM vendor, or empty for the deployment default. */
  llm_provider?: LlmProvider | '';
  /** The configured browser provider, or empty for the deployment default. */
  browser_provider?: Provider | '';
  /** The configured CAPTCHA solver; empty when solving is off. */
  captcha_solver?: CaptchaSolver;
  /** What this key would actually use right now, deployment defaults included. */
  effective: {
    /** The LLM endpoint in use. */
    baseUrl: string;
    /** The model in use. */
    model: string;
    /** Whether any LLM credential is available. */
    hasLlmKey: boolean;
  };
  /** True when the LLM key in play belongs to the deployment, not this key. */
  inherited: boolean;
  /** Whether this key has its own LLM credential stored. */
  has_openai_key: boolean;
  /** Every browser provider, what it needs configured, and whether it is. */
  providers: Array<{
    /** The provider. */
    id: Provider;
    /** Its display name. */
    label: string;
    /** The config fields it requires. */
    needs: string[];
    /** Whether those fields are set. */
    configured: boolean;
  }>;
}
