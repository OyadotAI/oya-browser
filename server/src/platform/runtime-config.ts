/**
 * Deployment-wide configuration — persisted to DB, editable by the operator.
 *
 * This is the fallback layer only. Per-tenant settings live in key-config.js,
 * keyed by the API key, because the API key is the identity for everything
 * else in this control plane. Resolution is key -> here -> environment.
 *
 * Fallback: local file (data/config.json) when Supabase is not configured.
 */

import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { db } from './db.ts';
import { assertSafeTarget } from './net-guard.ts';
import { dataPath } from './paths.ts';
import { HttpError } from './errors.ts';
import { Status } from './http-status.ts';
import { JSON_INDENT, MASKED_KEY_TAIL } from './constants.ts';

// OYA_DATA_DIR lets tests point at a scratch directory instead of writing
// through to the deployment's real state.
const CONFIG_PATH = dataPath('config.json');

let config: Record<string, any> = {};

// ── Load on startup ──

/** Read the settings table into memory; false when there is no DB or the read fails. */
async function loadFromDb() {
  if (!db) return false;
  try {
    return await readSettings();
  } catch (e) {
    console.error('[config] Failed to load from Supabase:', e.message);
    return false;
  }
}

/** Copy every row of the settings table into memory; throws when the read fails. */
async function readSettings() {
  const { data, error } = await db.from('settings').select('key, value');
  if (error) throw error;
  for (const row of data) {
    config[row.key] = row.value;
  }
  console.log(`[config] Loaded ${data.length} settings from Supabase`);
  return true;
}

/** Read data/config.json into memory; a missing or bad file leaves the config empty. */
function loadFromFile() {
  try {
    config = JSON.parse(readFileSync(CONFIG_PATH, 'utf8'));
    console.log('[config] Loaded settings from file');
  } catch {}
}

// Init: try DB first, fall back to file
loadFromDb()
  .then((ok) => {
    if (!ok) loadFromFile();
  })
  .catch((e) => {
    console.error('[config] Failed to load from DB:', e.message);
    loadFromFile();
  });

// ── Persistence ──

/** Upsert every setting into the settings table; failures are logged, not thrown. */
async function saveToDb() {
  if (!db) return;
  try {
    await upsertSettings();
  } catch (e) {
    console.error('[config] Failed to save to Supabase:', e.message);
  }
}

/** Upsert every setting as a row keyed by name; throws when the write fails. */
async function upsertSettings() {
  const rows = Object.entries(config).map(([key, value]) => ({
    key,
    value: String(value),
    updated_at: new Date().toISOString(),
  }));
  if (rows.length === 0) return;
  const { error } = await db.from('settings').upsert(rows, { onConflict: 'key' });
  if (error) throw error;
}

/** Write the settings to data/config.json, best effort. */
function saveToFile() {
  try {
    mkdirSync(dirname(CONFIG_PATH), { recursive: true });
    writeFileSync(CONFIG_PATH, JSON.stringify(config, null, JSON_INDENT));
  } catch {}
}

/** Persist to the DB when there is one, otherwise to the file. */
function save() {
  if (db) {
    saveToDb().catch((e) => console.error('[config] save error:', e.message));
  } else {
    saveToFile();
  }
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

  /** Get the actual OpenAI key (not masked) — used by chat-service */
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
  // through, and chat-service hands the response body back to the caller —
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
