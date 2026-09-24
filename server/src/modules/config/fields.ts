/**
 * The per-key settings a caller may set through POST /config, the browser
 * providers onboarding offers, and the defaults for each LLM provider.
 */
import { validateBaseUrl } from '../../platform/runtime-config.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { validateEcs, ecsEnv, viewEcs } from './ecs.ts';

/**
 * A closed set, rejected rather than ignored. The SDK types these fields, but the CLI's
 * key=value path, curl and any non-TypeScript client do not go through that, and an
 * unknown provider used to be stored verbatim and then silently fall back to the OpenAI
 * defaults, which reads as "my Gemini key is broken".
 * Lazily referenced so the choice lists below can stay where they read best.
 */
const oneOf = (choices, label) => (value) => {
  const allowed = choices();
  if (!allowed.includes(value)) {
    throw new HttpError(Status.BAD_REQUEST, `${label} must be one of: ${allowed.join(', ')}`);
  }
  return value;
};

/** How one per-key setting is stored and checked. */
export type Field = {
  /** Sealed at rest, masked on read. */
  secret?: boolean;
  /**
   * The environment variable this field stands in for, which is what makes
   * providers.js and captcha.js work unchanged: they read an env object, and
   * `envFor()` hands them one with the key's values on top.
   */
  envVar?: string;
  /** Normalises or rejects a value before it is stored. */
  validate?: (value: any) => unknown;
  /**
   * Holds an object rather than a string, stored as JSON: checked against the
   * stored value (so a masked credential sent back keeps the real one), spelled
   * as environment variables for envFor(), and read back with credentials masked.
   */
  object?: {
    /** Checks and normalises the object sent, given the one stored. */
    validate: (value: unknown, previous: any) => object;
    /** The environment variables the object stands in for. */
    toEnv: (value: any) => Record<string, string>;
    /** The object as get() shows it, credentials masked with `mask`. */
    view: (value: any, mask: (v: string) => string) => object;
  };
};

/** Every setting a key can hold through set(), and how each one is treated. */
export const FIELDS: Record<string, Field> = {
  llm_provider: { validate: oneOf(() => Object.keys(LLM_DEFAULTS), 'llm_provider') },
  openai_api_key: { secret: true, envVar: 'OPENAI_API_KEY' },
  openai_base_url: { validate: validateBaseUrl },
  chat_model: {},

  browser_provider: { validate: oneOf(() => PROVIDER_CHOICES.map((p) => p.id), 'browser_provider') },
  anchor_api_key: { secret: true, envVar: 'ANCHOR_API_KEY' },
  browserbase_api_key: { secret: true, envVar: 'BROWSERBASE_API_KEY' },
  browserbase_project_id: { envVar: 'BROWSERBASE_PROJECT_ID' },
  steel_api_key: { secret: true, envVar: 'STEEL_API_KEY' },
  browseruse_api_key: { secret: true, envVar: 'BROWSERUSE_API_KEY' },
  cdp_ws_url: { secret: true, envVar: 'OYA_CDP_WS_URL' },

  // Which runtime this key's Oya Cloud browsers run on. Docker and Kubernetes
  // only as the operator configured them; Daytona and ECS on the key's own
  // account once it sets that runtime's credentials: daytona_api_key, or ecs.auth (drivers/sandbox/tenancy.ts).
  sandbox_runtime: {
    envVar: 'OYA_CLOUD_RUNTIME',
    validate: oneOf(() => SANDBOX_RUNTIMES, 'sandbox_runtime'),
  },
  daytona_api_key: { secret: true, envVar: 'OYA_CLOUD_API_KEY' },
  daytona_snapshot: { envVar: 'OYA_CLOUD_SNAPSHOT' },
  daytona_target: { envVar: 'OYA_CLOUD_TARGET' },
  // Where this key's ECS browsers run and how Oya signs in to its AWS account.
  ecs: { secret: true, object: { validate: validateEcs, toEnv: ecsEnv, view: viewEcs } },

  captcha_solver: {
    envVar: 'OYA_CAPTCHA_PROVIDER',
    validate: oneOf(() => ['capsolver', '2captcha'], 'captcha_solver'),
  },
  captcha_api_key: { secret: true, envVar: 'OYA_CAPTCHA_API_KEY' },

  onboarded: {},
  // When a desktop Oya browser last enrolled or paired with this key. Cloud
  // browsers inherit that browser's logins, so until this is set the
  // dashboard tells the user to install it and sign in.
  desktop_seen_at: {},
  // The version the desktop last connected with, so a new one counts as an update.
  desktop_version: {},
  // When this key first started a browser, so the first one counts as activation.
  first_browser_at: {},
};

/**
 * The runtimes sandbox_runtime can name. Kept here rather than read from the
 * sandbox driver, which imports this module's service.
 */
export const SANDBOX_RUNTIMES = ['daytona', 'docker', 'k8s', 'ecs'];

/** What onboarding offers, in the order it offers it. */
export const PROVIDER_CHOICES = [
  { id: 'oya-selfhosted', label: 'Managed Docker browsers', needs: [] },
  { id: 'oya-cloud', label: 'Oya Browsers on Cloud', needs: [] },
  { id: 'browseruse', label: 'Browser Use Cloud', needs: ['browseruse_api_key'] },
  { id: 'browserbase', label: 'Browserbase', needs: ['browserbase_api_key'] },
  { id: 'steel', label: 'Steel', needs: ['steel_api_key'] },
  { id: 'anchor', label: 'Anchor', needs: ['anchor_api_key'] },
  { id: 'cdp', label: 'Your own Chrome (CDP)', needs: ['cdp_ws_url'] },
];

/** Sensible defaults per LLM provider, so onboarding is key + model and nothing else. */
export const LLM_DEFAULTS = {
  openai: { base: 'https://api.openai.com/v1', model: 'gpt-4o-mini' },
  // Claude's own Messages API (platform/llm/anthropic.ts): thinking, caching and parallel tool calls.
  anthropic: { base: 'https://api.anthropic.com/v1', model: 'claude-opus-5' },
  gemini: { base: 'https://generativelanguage.googleapis.com/v1beta/openai', model: 'gemini-3.8-flash' },
  // Gemini Enterprise (ex-Vertex AI) in express mode: a global endpoint with no project
  // or location, and an API key that only authenticates against native generateContent,
  // the OpenAI-compatible .../endpoints/openapi path wants an OAuth token instead. llm.js
  // translates for this base URL. A project-scoped enterprise endpoint still works: set
  // openai_base_url to .../endpoints/openapi and use an access token as the key.
  vertex: { base: 'https://aiplatform.googleapis.com/v1/publishers/google', model: 'gemini-2.5-flash' },
};
