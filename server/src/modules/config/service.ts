/**
 * Settings, keyed by API key.
 *
 * The API key is the identity for everything in this control plane, browsers,
 * personas, cookies, usage, so it is the identity for configuration too. A
 * key carries its own LLM credentials, its default browser provider, that
 * provider's credentials, and its CAPTCHA solver. Nothing is read from a
 * shared account, and nothing has to be in the environment: a self-hoster
 * onboards through the UI or `oya init` and never edits a compose file.
 *
 * Resolution is key -> host default (oya_browser.settings) -> environment, so
 * an operator who does want to configure a whole deployment centrally still
 * can, and a key that sets nothing behaves as it did before.
 *
 * Credentials are sealed with the envelope scheme in secrets.js. Reads return
 * masked values; only the server resolves the plaintext.
 */
import { managedConfigured } from '../control/managed.ts';
import { HttpError, invalid } from '../../platform/errors.ts';
import { CONFIG_VALUE_MAX_CHARS } from './constants.ts';
import { GOT_MAX_CHARS } from '../../platform/constants.ts';
import { Status } from '../../platform/http-status.ts';
import { fingerprint as ownerOf } from '../../platform/audit.ts';
import { sealText, openText } from '../../platform/secrets.ts';
import { runtimeConfig } from '../../platform/runtime-config.ts';
import { isConfigured as cloudConfigured, ecsExternalId } from '../../drivers/sandbox.ts';
import { FIELDS, PROVIDER_CHOICES, LLM_DEFAULTS } from './fields.ts';
import { llmCatalog } from './llm-catalog.ts';
import { store, state, scopeFor, flush, markChanged, changed } from './store.ts';
import { MASK_TAIL } from './constants.ts';
import { agentKeyUnclaimed } from '../auth/service.ts';

export { FIELDS, PROVIDER_CHOICES } from './fields.ts';

/** The llm_provider values, as a message names them: "anthropic" | "openai" | .... */
export const llmProviderList = () =>
  Object.keys(LLM_DEFAULTS)
    .map((p) => `"${p}"`)
    .join(' | ');
export { saveRouting, restoreRouting } from './routing.ts';
export { getSlack, saveSlack, clearSlack } from './slack.ts';
export { savePlaybook, deletePlaybook, listPlaybooks, getPlaybook, cleanupPlaybooks } from './playbooks.ts';
export { restore, drain } from './persistence.ts';
export { getSiteNotes, saveSiteNotes, allSiteNotes, forgetSiteNotes } from './site-notes.ts';

/** A secret as the dashboard sees it: a few trailing characters, or nothing. */
const mask = (v) => (v ? '••••' + String(v).slice(-MASK_TAIL) : '');

/** A stored value in plaintext; a secret that no longer unseals reads as empty. */
function reveal(owner, field, raw) {
  if (raw == null) return '';
  if (!FIELDS[field]?.secret) return raw;
  try {
    return openText(scopeFor(owner), raw);
  } catch {
    return '';
  }
}

/** Every field for one key, plaintext. Server-side only. */
function plain(apiKey) {
  const owner = ownerOf(apiKey);
  const row = store.get(owner) || {};
  const out: Record<string, any> = {};
  for (const field of Object.keys(FIELDS)) out[field] = reveal(owner, field, row[field]);
  return out;
}

/** A key's fields with every secret masked; an object setting keeps its shape, only its credentials masked. */
function masked(own) {
  const out = { ...own };
  for (const [field, spec] of Object.entries(FIELDS)) {
    if (spec.object) out[field] = own[field] ? spec.object.view(JSON.parse(own[field]), mask) : null;
    else if (spec.secret) out[field] = mask(own[field]);
  }
  return out;
}

/** Providers this server hosts itself, and how each says it is set up for a key. */
const HOSTED = { 'oya-selfhosted': () => managedConfigured(), 'oya-cloud': (apiKey) => cloudConfigured(apiKey) };

/** Whether a provider choice is usable for the key: hosted and set up, or its credentials are present. */
function configured(p, own, apiKey) {
  if (HOSTED[p.id]) return HOSTED[p.id](apiKey);
  return p.needs.every((f) => !!own[f] || !!process.env[FIELDS[f]?.envVar]);
}

/** The LLM settings a key would use, host defaults included. */
const llmView = (own, llm, host) => ({
  // What this key would actually use right now, host defaults included.
  effective: { baseUrl: llm.baseUrl, model: llm.model, hasLlmKey: !!llm.openaiKey },
  // True when the LLM key in play belongs to the deployment, not this key.
  inherited: !own.openai_api_key && !!llm.openaiKey,
  has_openai_key: !!llm.openaiKey,
  openai_base_url: own.openai_base_url || host.openai_base_url,
  chat_model: own.chat_model || llm.model,
});

/** The view with the ExternalId to trust beside ecs, shown whether or not ECS is set so the role can be made first. */
const withExternalId = (view, apiKey) => ({ ...view, ecs: { ...view.ecs, externalId: ecsExternalId(apiKey) } });

/** Masked view for the dashboard and `oya init`. */
export function get(apiKey) {
  const own: any = plain(apiKey);
  const llm = resolve(apiKey);
  return {
    ...withExternalId(masked(own), apiKey),
    ...llmView(own, llm, runtimeConfig.get()),
    providers: PROVIDER_CHOICES.map((p) => ({ ...p, configured: configured(p, own, apiKey) })),
    // Every client builds its provider and model pickers from this, so they always agree.
    llm_catalog: llmCatalog(),
  };
}

/** Refuses an update before any of it is applied: a setting that does not exist, or a value that is not text. */
function checkUpdates(updates: Record<string, unknown>) {
  for (const [field, value] of Object.entries(updates)) {
    if (!Object.hasOwn(FIELDS, field)) throw unknownSetting(field);
    if (value !== null && typeof value === 'object' && !FIELDS[field].object) throw invalid(field, 'a string', value);
    if (typeof value === 'string' && value.length > CONFIG_VALUE_MAX_CHARS)
      throw invalid(field, `at most ${CONFIG_VALUE_MAX_CHARS} characters`, 'a longer string');
  }
}

/** A setting nobody defined: named, with every setting there is, so a typo is found in one read. */
const unknownSetting = (field: string) =>
  new HttpError(
    Status.BAD_REQUEST,
    `Unknown setting "${field.slice(0, GOT_MAX_CHARS)}". Settings are: ${Object.keys(FIELDS).join(', ')}.`,
    {
      code: 'invalid_request',
      field,
    },
  );

/** Apply one field of an update to a row; false when the update leaves it alone. */
async function apply(owner, row, [field, spec], value) {
  if (value === undefined) return false;
  value = spec.object ? objectText(owner, row, field, value) : value === null ? '' : String(value);
  // Never write the masked placeholder back over a real credential.
  if (spec.secret && value.startsWith('•')) return false;
  if (spec.validate && value) value = await spec.validate(value);
  if (!value) delete row[field];
  else row[field] = spec.secret ? sealText(scopeFor(owner), value) : value;
  return true;
}

/**
 * An object setting as the JSON stored for it, checked against the stored value
 * (so a masked credential sent back keeps the real one); empty clears it.
 * JSON text is accepted too, for the CLI's key=value and other string-only clients.
 */
function objectText(owner, row, field, value) {
  if (value === null || value === '') return '';
  const sent = typeof value === 'string' ? parseObject(field, value) : value;
  const stored = reveal(owner, field, row[field]);
  return JSON.stringify(FIELDS[field].object.validate(sent, stored ? JSON.parse(stored) : null));
}

/** JSON text as an object, or a 400 naming the setting. */
function parseObject(field, text) {
  try {
    return JSON.parse(text);
  } catch {
    throw invalid(field, 'an object (or its JSON)', text);
  }
}

/** Keep a changed row and write it out in the background. */
function commit(owner, row) {
  store.set(owner, row);
  markChanged(owner);
  flush().catch((e) => console.error('[key-config] save failed:', e.message));
}

/**
 * Apply the FIELDS present in `updates`; null or empty clears one, and a masked
 * placeholder never overwrites a real secret. Returns whether anything changed.
 */
export async function set(apiKey, updates = {}) {
  checkUpdates(updates);
  const owner = ownerOf(apiKey);
  const row = { ...(store.get(owner) || {}) };
  let changed = false;
  for (const entry of Object.entries(FIELDS)) changed = (await apply(owner, row, entry, updates[entry[0]])) || changed;
  if (!changed) return false;
  commit(owner, row);
  return true;
}

/** The LLM settings a request runs with. */
type LlmConfig = {
  /** True when the credential is the key's own rather than the deployment's. */
  own?: boolean;
  /** The API key sent to the LLM provider. */
  openaiKey: string;
  /** The provider's OpenAI-compatible base URL. */
  baseUrl: string;
  /** The chat model to ask for. */
  model: string;
};

/** The LLM config of a key that brought its own credential. */
const ownLlm = (own, defaults): LlmConfig => ({
  own: true,
  openaiKey: own.openai_api_key,
  // A key's own base URL is only honoured alongside its own credential,
  // pairing a caller-supplied endpoint with the deployment's key would ship
  // that credential to an address the caller controls.
  baseUrl: own.openai_base_url || defaults.base,
  model: own.chat_model || defaults.model,
});

/** The LLM config of a key that runs on the deployment's credential. */
const hostLlm = (own): LlmConfig => ({
  openaiKey: runtimeConfig.getOpenAIKey(),
  baseUrl: runtimeConfig.getOpenAIBase(),
  model: own.chat_model || runtimeConfig.getChatModel(),
});

/** No LLM at all: an agent's unclaimed key brings its own rather than spend the deployment's. */
const noLlm = (own): LlmConfig => ({
  openaiKey: '',
  baseUrl: runtimeConfig.getOpenAIBase(),
  model: own.chat_model || '',
});

/** Effective LLM config for a request. */
export function resolve(apiKey): LlmConfig {
  const own: any = plain(apiKey);
  const defaults = LLM_DEFAULTS[own.llm_provider] || LLM_DEFAULTS.openai;
  if (own.openai_api_key) return ownLlm(own, defaults);
  return agentKeyUnclaimed(apiKey) ? noLlm(own) : hostLlm(own);
}

/**
 * process.env with this key's credentials layered on top. Anything that reads
 * configuration from the environment, providers.js, captcha.js, takes this
 * and becomes per-key without knowing that keys exist.
 */
export function envFor(apiKey, base = process.env) {
  const own = plain(apiKey);
  const env = { ...base };
  for (const [field, spec] of Object.entries(FIELDS)) {
    if (spec.envVar && own[field]) env[spec.envVar] = own[field];
    if (spec.object && own[field]) Object.assign(env, spec.object.toEnv(JSON.parse(own[field])));
  }
  return env;
}

/** Which provider a key's browsers come from when the caller does not say. */
export function providerFor(apiKey) {
  return plain(apiKey).browser_provider || process.env.OYA_BROWSER_PROVIDER || 'cdp';
}

/** Test hook. */
export function reset() {
  store.clear();
  state.dirty = false;
  changed.clear();
}
