/**
 * Which second factor each persona (and site) uses, sealed at rest in
 * mfa.json.
 *
 * TOTP seeds are credential material of the same weight as a password. They are
 * encrypted at rest with the shared envelope scheme, never logged, and never
 * returned by the API, only whether one is configured.
 */

import { readFileSync, writeFileSync, mkdirSync, renameSync } from 'fs';
import { dirname } from 'path';
import { sealText, openText } from '../../platform/secrets.ts';
import { assertSafeTarget } from '../../platform/net-guard.ts';
import { dataPath } from '../../platform/paths.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { totp } from './totp.ts';

/**
 * `personaId|domain` -> sealed config, or bare `personaId` for the persona-wide
 * default. One persona drives several portals and they do not agree on a factor,
 * an authenticator app here, an emailed code there, so the factor is filed
 * per site, and the persona-wide record is the fallback (and what every
 * mfa.json written before this keying was introduced still is).
 */
const configs = new Map();
/** Where the sealed factors live. */
const STORE = dataPath('mfa.json');
/** Owner-only directory, for the folder holding the store. */
const DIR_MODE = 0o700;
/** Owner-only file. */
const FILE_MODE = 0o600;

/** Load the stored factors from disk; a missing file means none. */
export function restore() {
  try {
    for (const [id, value] of Object.entries(JSON.parse(readFileSync(STORE, 'utf8')))) configs.set(id, value);
  } catch (e) {
    if (e.code !== 'ENOENT') throw new Error(`Cannot read MFA settings: ${e.message}`);
  }
}
/** Write the factors to disk atomically (temp file, then rename), owner-only. */
function persist() {
  mkdirSync(dirname(STORE), { recursive: true, mode: DIR_MODE });
  const temp = `${STORE}.${process.pid}.tmp`;
  writeFileSync(temp, JSON.stringify(Object.fromEntries(configs)), { mode: FILE_MODE });
  renameSync(temp, STORE);
}
restore();

/** The store key for a persona's factor on one site, or its persona-wide factor. */
const keyFor = (personaId, domain) => (domain ? `${personaId}|${domain}` : String(personaId));
/** The sealing scope for one store key. */
const scopeFor = (key) => `mfa:${key}`;

/** Factor kinds a persona can be given. */
export const TYPES = ['totp', 'email', 'sms', 'gmail', 'graph'];

/** A TOTP factor needs a secret that decodes. */
function checkTotp(config) {
  if (!config.secret) throw new HttpError(Status.BAD_REQUEST, 'a TOTP secret is required');
  totp(config.secret); // fail now, not at the login prompt
}

/**
 * Fixed vendor hostnames, so there is no SSRF surface to check here, but a
 * missing token is only discoverable at the login prompt otherwise.
 */
function checkMailbox(config) {
  const { type } = config;
  if (!config.refreshToken) throw new HttpError(Status.BAD_REQUEST, `a ${type} refreshToken is required`);
  if (!config.clientId) throw new HttpError(Status.BAD_REQUEST, `a ${type} clientId is required`);
}

/**
 * The relay URL is caller-supplied and the server fetches it, so it is an
 * SSRF primitive: rejected here so the tenant sees why, and again at fetch
 * time because a public name can be re-pointed at an internal address.
 */
async function checkRelay(config) {
  if (!config.url) throw new HttpError(Status.BAD_REQUEST, `a ${config.type} relay url is required`);
  await assertSafeTarget(config.url, { protocols: ['http:', 'https:'], label: 'mfa relay url' });
}

/** How each factor kind is validated. */
const CHECKS: Record<string, (config: any) => void | Promise<void>> = {
  totp: checkTotp,
  gmail: checkMailbox,
  graph: checkMailbox,
  email: checkRelay,
  sms: checkRelay,
};

/**
 * Store a factor for a persona, or for one site when `domain` is given.
 * Validated now so a bad secret or relay URL fails here, not at the login prompt.
 */
export async function set(personaId, config, domain = null) {
  const { type } = config || {};
  if (!TYPES.includes(type)) throw new HttpError(Status.BAD_REQUEST, `mfa type must be one of ${TYPES.join(', ')}`);
  const pending = CHECKS[type](config);
  if (pending) await pending; // only the relay check waits, as before
  const key = keyFor(personaId, domain);
  configs.set(key, sealText(scopeFor(key), config));
  persist();
  return describe(personaId, domain);
}

/** Remove one factor: the site's, or the persona-wide one when `domain` is omitted. */
export function clear(personaId, domain = null) {
  const removed = configs.delete(keyFor(personaId, domain));
  if (removed) persist();
  return removed;
}

/** Drop every factor for a persona, site-specific ones included. */
export function clearAll(personaId) {
  const owned = [...configs.keys()].filter((key) => key === personaId || key.startsWith(`${personaId}|`));
  for (const key of owned) configs.delete(key);
  if (owned.length) persist();
  return owned.length;
}

/** Whether a factor is configured, never what it is. */
export function describe(personaId, domain = null) {
  const found = resolve(personaId, domain);
  if (!found) return { configured: false };
  return { configured: true, type: found.config.type, ...(found.domain ? { domain: found.domain } : {}) };
}

/** Every site-specific factor this persona holds, types only. */
export function list(personaId) {
  const out = [];
  for (const key of configs.keys()) {
    if (!key.startsWith(`${personaId}|`)) continue;
    const domain = key.slice(personaId.length + 1);
    out.push({ domain, type: openText(scopeFor(key), configs.get(key)).type });
  }
  return out.sort((a, b) => a.domain.localeCompare(b.domain));
}

/** The site's own factor, else the persona-wide one. */
function resolve(personaId, domain) {
  for (const candidate of [domain ? keyFor(personaId, domain) : null, String(personaId)].filter(Boolean)) {
    const sealed = configs.get(candidate);
    if (sealed)
      return { config: openText(scopeFor(candidate), sealed), domain: candidate === String(personaId) ? null : domain };
  }
  return null;
}

/** The decrypted factor config that applies, or null. */
export function load(personaId, domain) {
  return resolve(personaId, domain)?.config || null;
}

/** Test hook. */
export function reset() {
  configs.clear();
}
