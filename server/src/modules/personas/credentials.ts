/**
 * Site credentials.
 *
 * The cookie jar is still the primary way a persona stays signed in, see
 * cookie-store.js. This is the fallback for portals that expire a session
 * server-side between runs and then demand a real login: an unattended run has
 * nothing to recover with unless the username and password are here.
 *
 * A password is credential material of the same weight as a TOTP seed, and it
 * is stored the same way: sealed at rest with the shared envelope scheme, never
 * logged, and never returned by the API, only the username it belongs to.
 */

import { sealText, openText } from '../../platform/secrets.ts';
import { dataPath } from '../../platform/paths.ts';
import { RecordTable, importLegacyFile } from '../../platform/storage/index.ts';
import { HttpError, invalid } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { REGISTRABLE_LABELS } from './constants.ts';

/** `personaId|domain` -> sealed { username, password } */
const configs = new Map();
/** Where the sealed records are kept. */
const table = new RecordTable('persona_credentials');
/** The file they lived in before storage drivers, imported once. */
const LEGACY_FILE = dataPath('credentials.json');

/** Load the sealed records from storage, taking in the legacy file first if it is still there. */
export async function restore() {
  await importLegacyFile(LEGACY_FILE, (records) => table.put(Object.entries(records)));
  configs.clear();
  for (const [id, value] of await table.load()) configs.set(id, value);
}

/**
 * The host a credential is filed under: lowercased, no `www.`, no port.
 *
 * Portals move between subdomains mid-flow (a login host, then an app host), so
 * lookup also tries the registrable parent, one credential for `example.com`
 * covers `login.example.com` without filing it twice.
 */
export function domainOf(url) {
  if (typeof url !== 'string') return null;
  const host = hostOf(url);
  if (!host || host === 'localhost') return host || null;
  const site = host.replace(/^www\./, '');
  return HOST_NAME.test(site) ? site : null;
}

/** The lower-cased host of a URL or bare host, or null when it does not parse. */
function hostOf(url) {
  try {
    return new URL(/^https?:\/\//i.test(url) ? url : `https://${url}`).hostname.toLowerCase();
  } catch {
    return null;
  }
}

/** A host name as a site: labels of letters, digits and hyphens, one or more. `..`, a space and a path are not one. */
const HOST_NAME = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$/;

/** The registrable parent of a subdomain (`login.example.com` -> `example.com`), or null. */
const parentOf = (domain) => {
  const parts = String(domain).split('.');
  return parts.length > REGISTRABLE_LABELS ? parts.slice(-REGISTRABLE_LABELS).join('.') : null;
};

/** The store key for a persona's site. */
const keyFor = (personaId, domain) => `${personaId}|${domain}`;
/** The sealing scope, so a record cannot be opened under another persona or site. */
const scopeFor = (personaId, domain) => `cred:${personaId}:${domain}`;

/** Seal and save a username and password for a persona's site. Returns the description, never the password. */
export async function set(personaId, domain, config) {
  if (!domain) throw new HttpError(Status.BAD_REQUEST, 'a domain is required');
  const site = domainOf(domain);
  if (!site) throw invalid('domain', 'a host name such as accounts.google.com', domain);
  const sealed = sealText(scopeFor(personaId, site), loginFrom(config));
  await table.put([[keyFor(personaId, site), sealed]]);
  configs.set(keyFor(personaId, site), sealed);
  return describe(personaId, site);
}

/** The username and password from `config`, both required. */
function loginFrom(config) {
  const username = String(config?.username ?? '');
  const password = String(config?.password ?? '');
  if (!username) throw new HttpError(Status.BAD_REQUEST, 'a username is required');
  if (!password) throw new HttpError(Status.BAD_REQUEST, 'a password is required');
  return { username, password };
}

/** Remove the credential filed under exactly this site. True if there was one. */
export async function clear(personaId, domain) {
  const key = keyFor(personaId, domainOf(domain));
  if (!configs.has(key)) return false;
  await table.remove([key]);
  return configs.delete(key);
}

/** Drop every credential for a persona. Called when the persona itself goes. */
export async function clearAll(personaId) {
  const keys = [...configs.keys()].filter((key) => key.startsWith(`${personaId}|`));
  await table.remove(keys);
  for (const key of keys) configs.delete(key);
  return keys.length;
}

/** Which account is bound to a site, never the password. */
export function describe(personaId, domain) {
  const site = domainOf(domain);
  const found = site && lookup(personaId, site);
  if (!found) return { configured: false };
  return { configured: true, domain: found.domain, username: found.username };
}

/** Every site this persona can sign in to, usernames only. */
export function list(personaId) {
  const out = [];
  for (const key of configs.keys()) {
    if (!key.startsWith(`${personaId}|`)) continue;
    const domain = key.slice(personaId.length + 1);
    out.push({ domain, username: open(personaId, domain).username });
  }
  return out.sort((a, b) => a.domain.localeCompare(b.domain));
}

/** Unseals a persona's credential for exactly this site. */
const open = (personaId, domain) => openText(scopeFor(personaId, domain), configs.get(keyFor(personaId, domain)));

/** The credential for a host, falling back to its registrable parent. */
export function lookup(personaId, domain) {
  const site = domainOf(domain);
  if (!site) return null;
  for (const candidate of [site, parentOf(site)].filter(Boolean)) {
    if (configs.has(keyFor(personaId, candidate))) {
      return { domain: candidate, ...open(personaId, candidate) };
    }
  }
  return null;
}

/** Test hook. */
export function reset() {
  configs.clear();
}
