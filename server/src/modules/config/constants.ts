/**
 * Every number key settings run on, by name: how secrets are masked and how the
 * fallback file is written.
 */

/** Trailing characters of a secret shown in its masked form. */
export const MASK_TAIL = 4;
/** Indent of the fallback settings file. */
export const FILE_INDENT = 2;
/** The fallback file holds sealed credentials: owner read and write only. */
export const FILE_MODE = 0o600;

/** Longest value one setting holds; longer is a paste gone wrong, not a model name or a key. */
export const CONFIG_VALUE_MAX_CHARS = 4096;
/** Provider setup sends one tool-free request rather than starting an agent run. */
export const MODEL_TEST_TIMEOUT_MS = 20_000;
/** Bound even a thinking provider's diagnostic response. */
export const MODEL_TEST_MAX_TOKENS = 1024;
/** Only explicit API-key providers are tested, never a deployment or subscription credential. */
export const MODEL_TEST_PROVIDERS = new Set(['gemini', 'anthropic', 'openrouter']);

/** Where OpenRouter lists its models. Set OYA_OPENROUTER_MODELS_URL to '' to use only the built-in list (offline hosts, tests). */
export const OPENROUTER_MODELS_URL = process.env.OYA_OPENROUTER_MODELS_URL ?? 'https://openrouter.ai/api/v1/models';
/** How long a fetched OpenRouter list is served before it is refreshed in the background. */
export const OPENROUTER_MODELS_TTL_MS = 3_600_000;
/** How long one fetch of OpenRouter's list may take. */
export const OPENROUTER_MODELS_TIMEOUT_MS = 10_000;
/** The most OpenRouter models a picker offers: enough for every family, few enough to scroll. */
export const OPENROUTER_MODELS_MAX = 150;

/** The operator's vendor credentials, which the hosted deployment never lends to a caller's browsers. */
export const OPERATOR_ONLY_ENV = new Set([
  'ANCHOR_API_KEY',
  'BROWSERBASE_API_KEY',
  'BROWSERBASE_PROJECT_ID',
  'STEEL_API_KEY',
  'BROWSERUSE_API_KEY',
  'OYA_CDP_WS_URL',
]);

/** The CAPTCHA solver's settings by key field: the operator's are lent to Oya Cloud browsers only. */
export const CAPTCHA_FIELDS: Record<string, string> = {
  captcha_api_key: 'OYA_CAPTCHA_API_KEY',
  captcha_solver: 'OYA_CAPTCHA_PROVIDER',
};
/** Browsers the operator's CAPTCHA solver pays for: the ones this server runs. */
export const CLOUD_ONLY_ENV_PROVIDERS = new Set(['oya-cloud', 'oya-selfhosted']);

/** Sign in with ChatGPT (plan usage for open-source and locally hosted apps): where the browser signs in. */
export const CHATGPT_AUTHORIZE_URL = 'https://auth.openai.com/api/accounts/authorize';
/** Where codes and refresh tokens are exchanged. */
export const CHATGPT_TOKEN_URL = 'https://auth.openai.com/api/accounts/oauth/token';
/** The API the plan's tokens are for. */
export const CHATGPT_RESOURCE = 'https://api.openai.com/v1';
/** What sign-in asks for; chatgpt.tokens.use.direct is the plan permission itself. */
export const CHATGPT_SCOPE = 'openid profile email offline_access resource.invoke chatgpt.tokens.use.direct';
/** The scope without which a token may sign in but not spend the plan. */
export const CHATGPT_PLAN_SCOPE = 'chatgpt.tokens.use.direct';
/** The client a first sign-in registers through; the issued client id is kept and reused after it. */
export const CHATGPT_DYNAMIC_CLIENT = 'dynamic_agent_client';
/** The name ChatGPT shows for this app when it is registered. */
export const CHATGPT_AGENT_NAME = 'Oya Browser';
/**
 * The loopback path sign-in returns to: the one path OpenAI's dynamic registration
 * accepts (every working client and its docs use it). It must never change once a
 * client is registered. The console's own sign-in returns to the same path, so the
 * server's handler only takes a return that carries OpenAI's parameters.
 */
export const CHATGPT_CALLBACK_PATH = '/auth/callback';
/** How long a started sign-in may take. */
export const CHATGPT_LOGIN_TTL_MS = 600_000;
/** Refresh an access token this long before it expires: a run keeps the token it started with, so leave it room. */
export const CHATGPT_REFRESH_EARLY_MS = 900_000;
/** A token request to OpenAI's sign-in gives up after this long. */
export const CHATGPT_TOKEN_TIMEOUT_MS = 30_000;
/** How much of OpenAI's reason for a refused token request is logged. */
export const CHATGPT_LOG_CHARS = 300;
/** How much of an issued client id (public, not a secret) is logged, to tell registrations apart. */
export const CHATGPT_ID_LOG_CHARS = 12;
/** Random bytes in a state, nonce or PKCE verifier. */
export const CHATGPT_SECRET_BYTES = 32;
/** The port this server listens on unless PORT says otherwise; sign-in's loopback callback names it. */
export const DEFAULT_PORT = 3100;
