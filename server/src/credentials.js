/**
 * Site credentials.
 *
 * The cookie jar is still the primary way a persona stays signed in — see
 * cookie-store.js. This is the fallback for portals that expire a session
 * server-side between runs and then demand a real login: an unattended run has
 * nothing to recover with unless the username and password are here.
 *
 * A password is credential material of the same weight as a TOTP seed, and it
 * is stored the same way: sealed at rest with the shared envelope scheme, never
 * logged, and never returned by the API — only the username it belongs to.
 */

import { sealText, openText } from './secrets.js';
import { readFileSync, writeFileSync, mkdirSync, renameSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

/** `personaId|domain` -> sealed { username, password } */
const configs = new Map();
const STORE = join(process.env.OYA_DATA_DIR || join(dirname(fileURLToPath(import.meta.url)), '..', 'data'), 'credentials.json');

export function restore() {
  try { for (const [id, value] of Object.entries(JSON.parse(readFileSync(STORE, 'utf8')))) configs.set(id, value); }
  catch (e) { if (e.code !== 'ENOENT') throw new Error(`Cannot read credentials: ${e.message}`); }
}
function persist() {
  mkdirSync(dirname(STORE), { recursive: true, mode: 0o700 });
  const temp = `${STORE}.${process.pid}.tmp`;
  writeFileSync(temp, JSON.stringify(Object.fromEntries(configs)), { mode: 0o600 });
  renameSync(temp, STORE);
}
restore();

/**
 * The host a credential is filed under: lowercased, no `www.`, no port.
 *
 * Portals move between subdomains mid-flow (a login host, then an app host), so
 * lookup also tries the registrable parent — one credential for `evicore.com`
 * covers `carecore.evicore.com` without filing it twice.
 */
export function domainOf(url) {
  let host;
  try { host = new URL(/^https?:\/\//i.test(url) ? url : `https://${url}`).hostname.toLowerCase(); }
  catch { return null; }
  if (!host || host === 'localhost') return host || null;
  return host.replace(/^www\./, '');
}

const parentOf = (domain) => {
  const parts = String(domain).split('.');
  return parts.length > 2 ? parts.slice(-2).join('.') : null;
};

const keyFor = (personaId, domain) => `${personaId}|${domain}`;
const scopeFor = (personaId, domain) => `cred:${personaId}:${domain}`;

export function set(personaId, domain, config) {
  const site = domainOf(domain);
  if (!site) throw Object.assign(new Error('a domain is required'), { status: 400 });
  const username = String(config?.username ?? '');
  const password = String(config?.password ?? '');
  if (!username) throw Object.assign(new Error('a username is required'), { status: 400 });
  if (!password) throw Object.assign(new Error('a password is required'), { status: 400 });
  configs.set(keyFor(personaId, site), sealText(scopeFor(personaId, site), { username, password }));
  persist();
  return describe(personaId, site);
}

export function clear(personaId, domain) {
  const site = domainOf(domain);
  const removed = configs.delete(keyFor(personaId, site));
  if (removed) persist();
  return removed;
}

/** Drop every credential for a persona. Called when the persona itself goes. */
export function clearAll(personaId) {
  let removed = 0;
  for (const key of [...configs.keys()]) {
    if (key.startsWith(`${personaId}|`)) { configs.delete(key); removed++; }
  }
  if (removed) persist();
  return removed;
}

/** Which account is bound to a site — never the password. */
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
export function reset() { configs.clear(); }
