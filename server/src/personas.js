/**
 * Personas.
 *
 * A persona is one identity: a fingerprint, a cookie jar and a proxy, bound
 * together and stable for its life. That binding is the whole point — one
 * account arriving from many device fingerprints is a bot-farm signal, and so
 * is many accounts arriving from one. Rotation means picking a different
 * persona, never giving a persona a new fingerprint.
 *
 * An API key groups personas; it is not itself one. Every key has a default
 * persona whose seed reproduces the fingerprint that key had before personas
 * existed, so nothing changes for a customer running a single account.
 *
 * Concurrency is capped per persona because one device cannot be in a thousand
 * places at once — at fleet scale that, not fingerprint diversity, is the
 * exposure.
 */

import { readFile, writeFile, mkdir } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { db } from './db.js';
import { fingerprint as ownerOf } from './audit.js';
import {
  getFingerprintForPersona, previewProfile, defaultPersonaSeed, newPersonaSeed,
} from './fingerprint.js';
export { prefsError } from './fingerprint.js';
import { metrics } from './metrics.js';
import * as mfa from './mfa.js';
import * as credentials from './credentials.js';
import * as proxies from './proxies.js';
import { summary as loginSummary, clear as clearLogin } from './cookie-store.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const STORE = process.env.OYA_DATA_DIR
  ? join(process.env.OYA_DATA_DIR, 'personas.json')
  : join(__dirname, '..', 'data', 'personas.json');

/**
 * A named persona is one device, so it gets a low cap — a person has a phone
 * and a laptop, not a thousand.
 */
const DEFAULT_MAX_CONCURRENT = Math.max(1, Number(process.env.OYA_PERSONA_MAX_CONCURRENT) || 2);

/**
 * The default persona is the migration path for keys that already run many
 * browsers, so it is uncapped unless the operator says otherwise. Those fleets
 * are exposed by concurrency rather than fingerprint diversity, which is what
 * the activeBrowsers metric surfaces; capping them here would break a working
 * setup on upgrade.
 */
const DEFAULT_PERSONA_MAX_CONCURRENT =
  Number(process.env.OYA_DEFAULT_PERSONA_MAX_CONCURRENT) || Infinity;

/** id -> persona. Personas are small and bounded by customer count. */
const personas = new Map();
/** id -> Set(browserId) currently running as it. */
const active = new Map();
let dirty = false;
let warnedFallback = false;

const shape = (p) => ({
  id: p.id,
  owner: p.owner,
  name: p.name,
  seed: p.seed,
  // Device choices made at creation. Immutable with the seed: together they
  // are the fingerprint, and the fingerprint is what must not change.
  prefs: p.prefs && typeof p.prefs === 'object' ? { ...p.prefs } : null,
  proxy: p.proxy || null,
  maxConcurrent: p.maxConcurrent === null ? Infinity : (p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT),
  isDefault: !!p.isDefault,
  createdAt: p.createdAt,
  lastUsedAt: p.lastUsedAt || null,
});

/** Public view: fingerprint included, seed and raw proxy credentials not. */
/** The operator's residential gateway, when this persona would fall back to it. */
function residentialExit(p) {
  if (p.proxy?.host) return null;
  const r = proxies.residential(p);
  return r ? { id: 'residential', label: 'Oya residential', geo: r.geo, healthy: true } : null;
}

export function describe(p) {
  const exit = proxies.assigned(p.id);
  return {
    id: p.id,
    name: p.name,
    isDefault: !!p.isDefault,
    createdAt: p.createdAt,
    lastUsedAt: p.lastUsedAt || null,
    activeBrowsers: active.get(p.id)?.size || 0,
    maxConcurrent: p.maxConcurrent === null ? Infinity : (p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT),
    proxy: p.proxy ? { host: p.proxy.host, port: p.proxy.port, geo: p.proxy.geo || null } : null,
    // The proxy it is actually on, when one has been assigned or pinned.
    exit: exit ? { id: exit.id, label: exit.label, geo: exit.geo, healthy: exit.available } : residentialExit(p),
    prefs: publicPrefs(p.prefs),
    fingerprint: describeProfile(fingerprintFor(p)),
    mfa: mfa.describe(p.id),
    // Per-site factors and credentials, so the dashboard can show which portals
    // this persona can sign in to on its own. Usernames and types only.
    sites: {
      mfa: mfa.list(p.id),
      credentials: credentials.list(p.id),
    },
    login: loginSummary(p.id),
  };
}

/** The public view of a fingerprint: enough to recognise the device, no seeds. */
export function describeProfile(fp) {
  return {
    platform: fp.navigator.platform,
    timezone: fp.timezone,
    locale: fp.locale,
    screen: `${fp.screen.width}x${fp.screen.height}`,
    webgl: fp.webgl.unmaskedRenderer,
    hardwareConcurrency: fp.navigator.hardwareConcurrency,
    deviceMemory: fp.navigator.deviceMemory,
    canvasSeed: fp.canvas.noiseSeed,
  };
}

/**
 * The key's default persona, created on first use. Its seed reproduces the
 * pre-persona fingerprint for that key exactly.
 */
export function defaultFor(apiKey) {
  const { id, seed } = defaultPersonaSeed(apiKey);
  let p = personas.get(id);
  if (!p) {
    p = shape({
      id, seed, owner: ownerOf(apiKey), name: 'Default', isDefault: true,
      maxConcurrent: DEFAULT_PERSONA_MAX_CONCURRENT,
      createdAt: new Date().toISOString(),
    });
    personas.set(id, p);
    dirty = true;
  }
  return p;
}

export function create(apiKey, { name, proxy, maxConcurrent, prefs, prefsChecked = true } = {}) {
  const { id, seed } = newPersonaSeed();
  const p = shape({
    id, seed, owner: ownerOf(apiKey),
    name: (name || id).slice(0, 100),
    prefs: markChecked(cleanPrefs(prefs), prefsChecked),
    proxy: proxy || null,
    maxConcurrent: Number(maxConcurrent) > 0 ? Number(maxConcurrent) : DEFAULT_MAX_CONCURRENT,
    createdAt: new Date().toISOString(),
  });
  personas.set(id, p);
  dirty = true;
  return p;
}

/**
 * Prefs validated at creation carry `checked`, inside prefs so it persists
 * wherever prefs do. Unmarked (older) prefs keep the rule they were created
 * under, or their device would move under its cookie jar (fingerprint.js).
 */
const markChecked = (prefs, checked) => (checked ? { ...(prefs || {}), checked: true } : prefs);

/** The device choices a caller made, without the internal marker. */
const publicPrefs = (prefs) => {
  if (!prefs) return null;
  const { checked, ...choices } = prefs;
  return Object.keys(choices).length ? choices : null;
};

/** Only the three device choices, only as strings. Anything else is dropped. */
function cleanPrefs(prefs) {
  if (!prefs || typeof prefs !== 'object') return null;
  const out = {};
  for (const k of ['platform', 'timezone', 'locale']) {
    if (typeof prefs[k] === 'string' && prefs[k] && prefs[k] !== 'auto') out[k] = prefs[k].slice(0, 64);
  }
  return Object.keys(out).length ? out : null;
}

/**
 * What a persona may change after creation: its label, its concurrency cap,
 * its proxy geo hint. Never seed or prefs — those are the device, and a
 * device that changes under an existing cookie jar is the tell this whole
 * model exists to avoid. Callers wanting a different device clone instead.
 */
export function update(apiKey, id, { name, maxConcurrent, proxy } = {}) {
  const p = get(apiKey, id);
  if (!p) return null;
  if (name !== undefined) p.name = String(name || p.id).slice(0, 100);
  if (maxConcurrent !== undefined) {
    if (p.isDefault && maxConcurrent !== null) {
      // The default persona's cap is a deployment decision, not a per-key one.
      p.maxConcurrent = Number(maxConcurrent) > 0 ? Number(maxConcurrent) : DEFAULT_PERSONA_MAX_CONCURRENT;
    } else {
      p.maxConcurrent = maxConcurrent === null || maxConcurrent === Infinity ? Infinity
        : (Number(maxConcurrent) > 0 ? Number(maxConcurrent) : DEFAULT_MAX_CONCURRENT);
    }
  }
  if (proxy !== undefined) p.proxy = proxy && typeof proxy === 'object' ? proxy : null;
  dirty = true;
  return p;
}

/** Same device choices, a fresh seed: a new machine of the same kind. */
export function clone(apiKey, id, { name } = {}) {
  const src = get(apiKey, id);
  if (!src) return null;
  return create(apiKey, {
    name: name || `${src.name} (copy)`,
    prefs: src.prefs,
    // Same kind of device as the source actually is, under the source's rule.
    prefsChecked: src.prefs?.checked === true,
    proxy: src.proxy,
    maxConcurrent: Number.isFinite(src.maxConcurrent) ? src.maxConcurrent : undefined,
  });
}

/** The fingerprint a persona created with these prefs would get. Persists nothing. */
export function preview(prefs) {
  const { id, seed } = newPersonaSeed();
  return previewProfile({ id: `preview-${id}`, seed, prefs: markChecked(cleanPrefs(prefs), true) });
}

export function list(apiKey) {
  const owner = ownerOf(apiKey);
  defaultFor(apiKey);   // always present
  return [...personas.values()].filter((p) => p.owner === owner);
}

export function get(apiKey, id) {
  const p = personas.get(id);
  // Ownership is the whole boundary: a persona id must not be usable by
  // another key, or one customer drives another's logged-in sessions.
  return p && p.owner === ownerOf(apiKey) ? p : null;
}

export function remove(apiKey, id) {
  const p = get(apiKey, id);
  if (!p) return false;
  if (p.isDefault) throw Object.assign(new Error('The default persona cannot be deleted'), { status: 400 });
  if ((active.get(id)?.size || 0) > 0) throw Object.assign(new Error('Persona is in use'), { status: 409 });
  personas.delete(id);
  clearLogin(id);
  // clearAll, not clear: a persona may hold a factor and a credential per
  // portal, and leaving those behind would outlive the identity they belong to.
  mfa.clearAll(id);
  credentials.clearAll(id);
  active.delete(id);
  dirty = true;
  return true;
}

/** Resolve what a browser should run as. Unknown or unowned ids are refused. */
export function resolve(apiKey, personaId) {
  if (!personaId || personaId === 'default') return defaultFor(apiKey);
  if (personaId === 'auto') {
    const owned = list(apiKey)
      .filter((p) => (active.get(p.id)?.size || 0) < (p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT))
      .sort((a, b) => String(a.lastUsedAt || '').localeCompare(String(b.lastUsedAt || '')));
    if (!owned.length) throw Object.assign(new Error('Every persona is at its concurrency cap'), { status: 429 });
    return owned[0];
  }
  const p = get(apiKey, personaId);
  if (!p) throw Object.assign(new Error(`No such persona: ${personaId}`), { status: 404 });
  return p;
}

export function fingerprintFor(persona) {
  return getFingerprintForPersona({ id: persona.id, seed: persona.seed, prefs: persona.prefs, proxy: persona.proxy });
}

/**
 * Take a concurrency slot. Refused past the cap rather than silently allowed:
 * a single device running more sessions than a person plausibly could is a
 * signal no amount of fingerprint work hides.
 */
export function acquire(persona, browserId) {
  if (!active.has(persona.id)) active.set(persona.id, new Set());
  const running = active.get(persona.id);
  const cap = persona.maxConcurrent ?? DEFAULT_MAX_CONCURRENT;
  if (!running.has(browserId) && running.size >= cap) {
    metrics.personaCapped.inc({});
    throw Object.assign(
      new Error(`Persona "${persona.name}" already has ${running.size} of ${cap} browsers running`),
      { status: 429 },
    );
  }
  running.add(browserId);
  persona.lastUsedAt = new Date().toISOString();
  dirty = true;
  metrics.personasActive.set({}, [...active.values()].reduce((n, s) => n + s.size, 0));
  return persona;
}

export function release(persona, browserId) {
  const running = active.get(persona?.id);
  if (!running) return;
  running.delete(browserId);
  if (!running.size) active.delete(persona.id);
  metrics.personasActive.set({}, [...active.values()].reduce((n, s) => n + s.size, 0));
}

export const activeCount = (id) => active.get(id)?.size || 0;

// ── Persistence ──
// Personas are the durable half of an identity; losing them would orphan the
// cookie jars keyed by them.

async function flush() {
  if (!dirty) return;
  dirty = false;
  const rows = [...personas.values()];
  const serialise = (p) => ({ ...p, maxConcurrent: Number.isFinite(p.maxConcurrent) ? p.maxConcurrent : null });
  try {
    if (db) {
      const { error } = await db.from('personas').upsert(
        rows.map((p) => ({
          id: p.id, owner: p.owner, name: p.name, seed: p.seed, prefs: p.prefs, proxy: p.proxy,
          max_concurrent: Number.isFinite(p.maxConcurrent) ? p.maxConcurrent : null,
          is_default: p.isDefault, created_at: p.createdAt, last_used_at: p.lastUsedAt,
          updated_at: new Date().toISOString(),
        })), { onConflict: 'id' });
      if (error) throw new Error(error.message);
    } else {
      await mkdir(dirname(STORE), { recursive: true });
      await writeFile(STORE, JSON.stringify(rows.map(serialise), null, 2), { mode: 0o600 });
    }
  } catch (e) {
    if (!warnedFallback) {
      console.error(`[personas] database write failed (${e.message}) — falling back to ${STORE}`);
      warnedFallback = true;
    }
    try {
      await mkdir(dirname(STORE), { recursive: true });
      await writeFile(STORE, JSON.stringify(rows.map(serialise), null, 2), { mode: 0o600 });
    } catch (fileErr) {
      dirty = true;
      console.error('[personas] file fallback failed:', fileErr.message);
    }
  }
}

async function fromFile() {
  for (const row of JSON.parse(await readFile(STORE, 'utf8'))) personas.set(row.id, shape(row));
}

export async function restore() {
  try {
    if (!db) return await fromFile().then(report);
    const { data, error } = await db.from('personas').select('*');
    if (error) throw new Error(error.message);
    for (const row of data || []) personas.set(row.id, shape({
      ...row, maxConcurrent: row.max_concurrent, isDefault: row.is_default,
      createdAt: row.created_at, lastUsedAt: row.last_used_at,
    }));
    report();
  } catch (e) {
    // A missing table or an unreachable database must not lose personas —
    // the cookie jars are keyed by them.
    if (db) console.error(`[personas] database read failed (${e.message}) — falling back to ${STORE}`);
    try { await fromFile(); report(); }
    catch (fileErr) { if (fileErr.code !== 'ENOENT') console.error('[personas] restore failed:', fileErr.message); }
  }
}

function report() {
  if (personas.size) console.log(`[personas] restored ${personas.size}`);
}

const timer = setInterval(() => flush().catch(() => {}), 10_000);
timer.unref?.();
export async function drain() { clearInterval(timer); await flush(); }

/** Test hook. */
export function reset() { personas.clear(); active.clear(); dirty = false; }
