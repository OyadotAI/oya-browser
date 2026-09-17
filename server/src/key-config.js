import { managedConfigured } from './control/managed.js';
/**
 * Settings, keyed by API key.
 *
 * The API key is the identity for everything in this control plane — browsers,
 * personas, cookies, usage — so it is the identity for configuration too. A
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

import { readFile, writeFile, mkdir } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { db } from './db.js';
import { fingerprint as ownerOf } from './audit.js';
import { sealText, openText } from './secrets.js';
import { runtimeConfig, validateBaseUrl } from './runtime-config.js';
import { isConfigured as cloudConfigured } from './sandbox.js';
import { cleanPlaybook } from './playbook-cleanup.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const STORE = process.env.OYA_DATA_DIR
  ? join(process.env.OYA_DATA_DIR, 'key-settings.json')
  : join(__dirname, '..', 'data', 'key-settings.json');

/**
 * secret  — sealed at rest, masked on read.
 * envVar  — the environment variable this field stands in for, which is what
 *           makes providers.js and captcha.js work unchanged: they read an env
 *           object, and `envFor()` hands them one with the key's values on top.
 */
/**
 * A closed set, rejected rather than ignored. The SDK types these fields, but the CLI's
 * key=value path, curl and any non-TypeScript client do not go through that — and an
 * unknown provider used to be stored verbatim and then silently fall back to the OpenAI
 * defaults, which reads as "my Gemini key is broken".
 * Lazily referenced so the choice lists below can stay where they read best.
 */
const oneOf = (choices, label) => (value) => {
  const allowed = choices();
  if (!allowed.includes(value)) {
    throw Object.assign(new Error(`${label} must be one of: ${allowed.join(', ')}`), { status: 400 });
  }
  return value;
};

export const FIELDS = {
  llm_provider:           { validate: oneOf(() => Object.keys(LLM_DEFAULTS), 'llm_provider') },
  openai_api_key:         { secret: true, envVar: 'OPENAI_API_KEY' },
  openai_base_url:        { validate: validateBaseUrl },
  chat_model:             {},

  browser_provider:       { validate: oneOf(() => PROVIDER_CHOICES.map((p) => p.id), 'browser_provider') },
  anchor_api_key:         { secret: true, envVar: 'ANCHOR_API_KEY' },
  browserbase_api_key:    { secret: true, envVar: 'BROWSERBASE_API_KEY' },
  browserbase_project_id: { envVar: 'BROWSERBASE_PROJECT_ID' },
  steel_api_key:          { secret: true, envVar: 'STEEL_API_KEY' },
  browseruse_api_key:     { secret: true, envVar: 'BROWSERUSE_API_KEY' },
  cdp_ws_url:             { secret: true, envVar: 'OYA_CDP_WS_URL' },

  captcha_solver:         { envVar: 'OYA_CAPTCHA_PROVIDER', validate: oneOf(() => ['capsolver', '2captcha'], 'captcha_solver') },
  captcha_api_key:        { secret: true, envVar: 'OYA_CAPTCHA_API_KEY' },

  onboarded:              {},
  // When a desktop Oya browser last enrolled or paired with this key. Cloud
  // browsers inherit that browser's logins, so until this is set the
  // dashboard tells the user to install it and sign in.
  desktop_seen_at:        {},
};

/** What onboarding offers, in the order it offers it. */
export const PROVIDER_CHOICES = [
  { id: 'oya-selfhosted', label: 'Managed Docker browsers', needs: [] },
  { id: 'oya-cloud',      label: 'Oya Browsers on Cloud',  needs: [] },
  { id: 'browseruse',     label: 'Browser Use Cloud',      needs: ['browseruse_api_key'] },
  { id: 'browserbase',    label: 'Browserbase',            needs: ['browserbase_api_key'] },
  { id: 'steel',          label: 'Steel',                  needs: ['steel_api_key'] },
  { id: 'anchor',         label: 'Anchor',                 needs: ['anchor_api_key'] },
  { id: 'cdp',            label: 'Your own Chrome (CDP)', needs: ['cdp_ws_url'] },
];

/** Sensible defaults per LLM provider, so onboarding is key + model and nothing else. */
const LLM_DEFAULTS = {
  openai:    { base: 'https://api.openai.com/v1',    model: 'gpt-4o-mini' },
  // Anthropic's OpenAI-compatible endpoint, so one client path covers both.
  anthropic: { base: 'https://api.anthropic.com/v1', model: 'claude-sonnet-4-5' },
  gemini:    { base: 'https://generativelanguage.googleapis.com/v1beta/openai', model: 'gemini-3.8-flash' },
  // Gemini Enterprise (ex-Vertex AI) in express mode: a global endpoint with no project
  // or location, and an API key that only authenticates against native generateContent —
  // the OpenAI-compatible .../endpoints/openapi path wants an OAuth token instead. llm.js
  // translates for this base URL. A project-scoped enterprise endpoint still works: set
  // openai_base_url to .../endpoints/openapi and use an access token as the key.
  vertex:    { base: 'https://aiplatform.googleapis.com/v1/publishers/google', model: 'gemini-2.5-flash' },
};

/** owner -> { field: value }. Secret fields hold sealed base64. */
const store = new Map();
let dirty = false;

const scopeFor = (owner) => `key-settings:${owner}`;
const mask = (v) => (v ? '••••' + String(v).slice(-4) : '');

function reveal(owner, field, raw) {
  if (raw == null) return '';
  if (!FIELDS[field]?.secret) return raw;
  try { return openText(scopeFor(owner), raw); } catch { return ''; }
}

/** Every field for one key, plaintext. Server-side only. */
function plain(apiKey) {
  const owner = ownerOf(apiKey);
  const row = store.get(owner) || {};
  const out = {};
  for (const field of Object.keys(FIELDS)) out[field] = reveal(owner, field, row[field]);
  return out;
}

/** Masked view for the dashboard and `oya init`. */
export function get(apiKey) {
  const own = plain(apiKey);
  const host = runtimeConfig.get();
  const out = { ...own };
  for (const [field, spec] of Object.entries(FIELDS)) {
    if (spec.secret) out[field] = mask(own[field]);
  }
  const llm = resolve(apiKey);
  return {
    ...out,
    // What this key would actually use right now, host defaults included.
    effective: { baseUrl: llm.baseUrl, model: llm.model, hasLlmKey: !!llm.openaiKey },
    // True when the LLM key in play belongs to the deployment, not this key.
    inherited: !own.openai_api_key && !!llm.openaiKey,
    has_openai_key: !!llm.openaiKey,
    openai_base_url: own.openai_base_url || host.openai_base_url,
    chat_model: own.chat_model || llm.model,
    providers: PROVIDER_CHOICES.map((p) => ({
      ...p,
      configured: p.id === 'oya-selfhosted' ? managedConfigured() : p.id === 'oya-cloud' ? cloudConfigured() : p.needs.every((f) => !!own[f] || !!process.env[FIELDS[f]?.envVar]),
    })),
  };
}

export async function set(apiKey, updates = {}) {
  const owner = ownerOf(apiKey);
  const row = { ...(store.get(owner) || {}) };
  let changed = false;
  for (const [field, spec] of Object.entries(FIELDS)) {
    let value = updates[field];
    if (value === undefined) continue;
    value = value === null ? '' : String(value);
    // Never write the masked placeholder back over a real credential.
    if (spec.secret && value.startsWith('•')) continue;
    if (spec.validate && value) value = await spec.validate(value);
    if (!value) delete row[field];
    else row[field] = spec.secret ? sealText(scopeFor(owner), value) : value;
    changed = true;
  }
  if (!changed) return false;
  store.set(owner, row);
  dirty = true;
  flush().catch((e) => console.error('[key-config] save failed:', e.message));
  return true;
}

/** Effective LLM config for a request. */
export function resolve(apiKey) {
  const own = plain(apiKey);
  const defaults = LLM_DEFAULTS[own.llm_provider] || LLM_DEFAULTS.openai;
  if (own.openai_api_key) {
    return {
      own: true,
      openaiKey: own.openai_api_key,
      // A key's own base URL is only honoured alongside its own credential —
      // pairing a caller-supplied endpoint with the deployment's key would ship
      // that credential to an address the caller controls.
      baseUrl: own.openai_base_url || defaults.base,
      model: own.chat_model || defaults.model,
    };
  }
  return {
    openaiKey: runtimeConfig.getOpenAIKey(),
    baseUrl: runtimeConfig.getOpenAIBase(),
    model: own.chat_model || runtimeConfig.getChatModel(),
  };
}

/**
 * process.env with this key's credentials layered on top. Anything that reads
 * configuration from the environment — providers.js, captcha.js — takes this
 * and becomes per-key without knowing that keys exist.
 */
export function envFor(apiKey, base = process.env) {
  const own = plain(apiKey);
  const env = { ...base };
  for (const [field, spec] of Object.entries(FIELDS)) {
    if (spec.envVar && own[field]) env[spec.envVar] = own[field];
  }
  return env;
}

/** Which provider a key's browsers come from when the caller does not say. */
export function providerFor(apiKey) {
  return plain(apiKey).browser_provider
    || process.env.OYA_BROWSER_PROVIDER
    || 'cdp';
}

// Private routing settings use the existing encrypted key-settings store.
// They are deliberately absent from FIELDS, so /config cannot overwrite them.
export async function saveRouting(apiKey, pool) {
  const owner = ownerOf(apiKey);
  const value = JSON.stringify({ providers: pool.configs(owner), strategy: pool.strategyFor(owner) });
  store.set(owner, { ...(store.get(owner) || {}), _routing: sealText(scopeFor(owner), value) });
  dirty = true;
  await flush();
}

export function restoreRouting(pool) {
  for (const [owner, fields] of store) {
    if (!fields._routing) continue;
    try {
      const saved = JSON.parse(openText(scopeFor(owner), fields._routing));
      for (const cfg of saved.providers || []) pool.register({ ...cfg, owner });
      if (saved.strategy) pool.setStrategy(owner, saved.strategy);
    } catch (err) { console.error('[routing] restore failed:', err.message); }
  }
}

// The Slack install for this key: bot token, workspace and the channel notifications
// go to. Sealed like routing and, like routing, kept out of FIELDS — POST /config
// must not be able to overwrite a bot token, and this is an object, not a scalar.
// Both ways in (OAuth install and a pasted bot token) write this one row.
export function getSlack(apiKey) {
  const owner = ownerOf(apiKey);
  const sealed = store.get(owner)?._slack;
  if (!sealed) return null;
  // Like reveal(): a row sealed under a rotated secret reads as absent rather than throwing.
  try { return JSON.parse(openText(scopeFor(owner), sealed)); } catch { return null; }
}

export async function saveSlack(apiKey, install) {
  const owner = ownerOf(apiKey);
  store.set(owner, { ...(store.get(owner) || {}), _slack: sealText(scopeFor(owner), JSON.stringify(install)) });
  dirty = true;
  await flush();
}

export async function clearSlack(apiKey) {
  const owner = ownerOf(apiKey);
  const row = { ...(store.get(owner) || {}) };
  delete row._slack;
  store.set(owner, row);
  dirty = true;
  await flush();
}

// Playbooks ride the same sealed store as routing, one row each; their steps can
// hold whatever the user typed. ponytail: in-memory per replica like the rest of
// this store; move to a table if playbooks must appear on other replicas without a restart.
const playbookField = (name) => `_playbook:${name}`;

export async function savePlaybook(apiKey, name, playbook) {
  const owner = ownerOf(apiKey);
  store.set(owner, { ...(store.get(owner) || {}), [playbookField(name)]: sealText(scopeFor(owner), JSON.stringify(playbook)) });
  dirty = true;
  await flush();
}

export async function deletePlaybook(apiKey, name) {
  const owner = ownerOf(apiKey);
  const row = { ...(store.get(owner) || {}) };
  delete row[playbookField(name)];
  store.set(owner, row);
  dirty = true;
  await flush();
}

/** Every playbook and draft this key saved, named by the field they are stored under. */
export function listPlaybooks(apiKey) {
  const owner = ownerOf(apiKey);
  const prefix = playbookField('');
  return Object.entries(store.get(owner) || {})
    .filter(([field]) => field.startsWith(prefix))
    .map(([field]) => getPlaybook(apiKey, field.slice(prefix.length)))
    .filter(Boolean);
}

export function getPlaybook(apiKey, name) {
  const owner = ownerOf(apiKey);
  const sealed = store.get(owner)?.[playbookField(name)];
  if (!sealed) return null;
  // Like reveal(): a row sealed under a rotated secret reads as absent instead of failing every listing.
  try { return { ...JSON.parse(openText(scopeFor(owner), sealed)), name }; } catch { return null; }
}

/** Upgrade saved playbooks and healed drafts without losing their sealed originals. */
export async function cleanupPlaybooks() {
  let updated = 0, unreadable = 0;
  for (const [owner, row] of store) {
    for (const [field, sealed] of Object.entries(row)) {
      if (!field.startsWith('_playbook:')) continue;
      let original;
      try { original = JSON.parse(openText(scopeFor(owner), sealed)); }
      catch { unreadable++; continue; }
      const cleaned = cleanPlaybook(original);
      if (JSON.stringify(cleaned) === JSON.stringify(original)) continue;
      const replacement = sealText(scopeFor(owner), JSON.stringify(cleaned));
      const backup = `_playbook-backup:v1:${field.slice('_playbook:'.length)}`;
      if (!Object.hasOwn(row, backup)) row[backup] = sealed;
      row[field] = replacement;
      dirty = true;
      updated++;
    }
  }
  if (updated) await flush();
  return { updated, unreadable };
}

// ── Persistence ──

let writeQueue = Promise.resolve();
function flush() {
  writeQueue = writeQueue.catch(() => {}).then(flushOnce);
  return writeQueue;
}
async function flushOnce() {
  if (!dirty) return;
  dirty = false;
  const rows = [...store.entries()].flatMap(([owner, fields]) =>
    Object.entries(fields).map(([key, value]) => ({ owner, key, value })));
  try {
    if (db) {
      // Deleting first is what makes a cleared field actually clear; upsert
      // alone would leave the old row behind.
      const owners = [...store.keys()];
      if (owners.length) {
        const { error } = await db.from('key_settings').delete().in('owner', owners);
        if (error) throw new Error(error.message);
      }
      if (rows.length) {
        const { error } = await db.from('key_settings')
          .upsert(rows.map((r) => ({ ...r, updated_at: new Date().toISOString() })),
            { onConflict: 'owner,key' });
        if (error) throw new Error(error.message);
      }
      return;
    }
  } catch (e) {
    console.error(`[key-config] database write failed (${e.message}) — falling back to ${STORE}`);
  }
  try {
    await mkdir(dirname(STORE), { recursive: true });
    await writeFile(STORE, JSON.stringify([...store], null, 2), { mode: 0o600 });
  } catch (e) {
    dirty = true;
    console.error('[key-config] file fallback failed:', e.message);
  }
}

export async function restore() {
  const load = (entries) => { for (const [owner, fields] of entries) store.set(owner, fields); };
  try {
    if (db) {
      const { data, error } = await db.from('key_settings').select('owner, key, value');
      if (error) throw new Error(error.message);
      for (const row of data || []) {
        const cur = store.get(row.owner) || {};
        cur[row.key] = row.value;
        store.set(row.owner, cur);
      }
      if (store.size) console.log(`[key-config] restored settings for ${store.size} keys`);
      await cleanupPlaybooks();
      return;
    }
  } catch (e) {
    console.error(`[key-config] database read failed (${e.message}) — falling back to ${STORE}`);
  }
  try { load(JSON.parse(await readFile(STORE, 'utf8'))); }
  catch (e) { if (e.code !== 'ENOENT') console.error('[key-config] restore failed:', e.message); }
  await cleanupPlaybooks();
}

export async function drain() { await flush(); }

/** Test hook. */
export function reset() { store.clear(); dirty = false; }
