/**
 * Deployment-wide configuration, persisted to storage, editable by the operator.
 *
 * This is the fallback layer only. Per-tenant settings live in key-config.js,
 * keyed by the API key, because the API key is the identity for everything
 * else in this control plane. Resolution is key -> here -> environment.
 */

import { getConnection, importLegacyFile } from './storage/index.ts';
import { assertSafeTarget } from './net-guard.ts';
import { dataPath } from './paths.ts';
import { HttpError } from './errors.ts';
import { Status } from './http-status.ts';
import { MASKED_KEY_TAIL } from './constants.ts';

/** The settings file from before storage drivers, imported once. */
const CONFIG_PATH = dataPath('config.json');

const config: Record<string, any> = {};

/** Settings as settings-table rows. */
const rowsOf = (settings: Record<string, any>) =>
  Object.entries(settings).map(([key, value]) => ({ key, value: String(value), updated_at: new Date().toISOString() }));

/** Read the settings table into memory, taking in the legacy config.json once. */
async function load() {
  const db = getConnection();
  await importLegacyFile(CONFIG_PATH, (legacy) => db.upsert('settings', rowsOf(legacy)));
  // A setting changed while this read was in flight is newer than the stored one: keep it.
  for (const row of await db.select('settings')) if (!Object.hasOwn(config, row.key)) config[row.key] = row.value;
}

// A failed read is logged; the environment still answers every setting.
load().catch((e) => console.error('[config] load failed:', e.message));

/** Write every setting to the settings table; a failure is logged, and the next change writes them all again. */
function save() {
  getConnection()
    .upsert('settings', rowsOf(config), { update: true })
    .catch((e) => console.error('[config] save failed:', e.message));
}

// ── Public API (unchanged signatures) ──

/** The saved OpenAI key, else the environment's, masked to its last few characters; empty when neither is set. */
function maskedOpenAIKey() {
  const key = config.openai_api_key || process.env.OPENAI_API_KEY;
  return key ? '••••' + key.slice(-MASKED_KEY_TAIL) : '';
}

/** Read and update the deployment-wide LLM settings, falling back to the environment. */
export const runtimeConfig = {
  get() {
    return {
      openai_api_key: maskedOpenAIKey(),
      openai_base_url: config.openai_base_url || process.env.OPENAI_BASE_URL || '',
      chat_model: config.chat_model || process.env.CHAT_MODEL || 'gpt-4o-mini',
      has_openai_key: !!(config.openai_api_key || process.env.OPENAI_API_KEY),
    };
  },

  async set(updates) {
    if (updates.openai_api_key !== undefined && !updates.openai_api_key.startsWith('••••')) {
      config.openai_api_key = updates.openai_api_key;
    }
    if (updates.openai_base_url !== undefined) config.openai_base_url = await validateBaseUrl(updates.openai_base_url);
    if (updates.chat_model !== undefined) config.chat_model = updates.chat_model;
    save();
  },

  /** Get the actual OpenAI key (not masked), used by chat-service */
  getOpenAIKey() {
    return config.openai_api_key || process.env.OPENAI_API_KEY || '';
  },

  getOpenAIBase() {
    return config.openai_base_url || process.env.OPENAI_BASE_URL || 'https://api.openai.com/v1';
  },

  getChatModel() {
    return config.chat_model || process.env.CHAT_MODEL || 'gpt-4o-mini';
  },
};

/**
 * The control plane fetches whatever base URL a tenant saves, so an
 * unvalidated value is a server-side request forgery primitive: cloud metadata,
 * internal services, anything routable from this host.
 */
export async function validateBaseUrl(value) {
  const raw = String(value).trim();
  if (!raw) return '';
  const url = parseBaseUrl(raw);
  checkBaseUrlShape(url);
  // The same resolving guard wsUrl, proxies and webhooks go through. A
  // hostname-only check let a public name with an A record of 10.0.0.5
  // through, and chat-service hands the response body back to the caller,
  // a read SSRF, not a blind one.
  await assertSafeTarget(url.href, { protocols: ['https:'], label: 'openai_base_url' });
  return url.href.replace(/\/+$/, '');
}

/** The base URL parsed, or a 400. */
function parseBaseUrl(raw) {
  try {
    return new URL(raw);
  } catch {
    throw new HttpError(Status.BAD_REQUEST, 'openai_base_url must be a valid URL');
  }
}

/** A 400 unless the URL is https with no credentials and no fragment. */
function checkBaseUrlShape(url) {
  const reject = (why) => {
    throw new HttpError(Status.BAD_REQUEST, `openai_base_url ${why}`);
  };
  if (url.protocol !== 'https:') reject('must use https');
  if (url.username || url.password) reject('must not embed credentials');
  if (url.hash) reject('must not contain a fragment');
}
