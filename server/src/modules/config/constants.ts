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

/** Where OpenRouter lists its models. Set OYA_OPENROUTER_MODELS_URL to '' to use only the built-in list (offline hosts, tests). */
export const OPENROUTER_MODELS_URL = process.env.OYA_OPENROUTER_MODELS_URL ?? 'https://openrouter.ai/api/v1/models';
/** How long a fetched OpenRouter list is served before it is refreshed in the background. */
export const OPENROUTER_MODELS_TTL_MS = 3_600_000;
/** How long one fetch of OpenRouter's list may take. */
export const OPENROUTER_MODELS_TIMEOUT_MS = 10_000;
/** The most OpenRouter models a picker offers: enough for every family, few enough to scroll. */
export const OPENROUTER_MODELS_MAX = 150;
