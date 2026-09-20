/**
 * Proxy pool.
 *
 * Rotation without exit-IP rotation is not rotation: distinct fingerprints all
 * arriving from one address correlate just as well as one fingerprint would.
 *
 * A proxy is assigned to a persona and stays with it. Stickiness is the point —
 * a logged-in session that returns from a different country looks like an
 * account takeover, which is the opposite of what a customer wants.
 *
 * Structure mirrors routing.js (cooldown on failure, least-used selection,
 * health) rather than inventing a second shape for the same problem.
 *
 * This file is the module's facade; registration, the health-check tunnel,
 * coherence and the residential gateway each live in their own file.
 */

import { openText } from '../../platform/secrets.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { proxies, assignments, scopeFor } from './store.ts';
import { getVia } from './tunnel.ts';
import { DEFAULT_CHECK_URL } from './constants.ts';

export { register } from './registration.ts';
export { coherence } from './coherence.ts';
export { residential } from './residential.ts';

/** Decrypted connection details. Server-side only. */
export function credentials(proxy) {
  return openText(scopeFor(proxy.id), proxy.sealed);
}

/** Delete one of the owner's proxies and drop its persona assignments. Shared proxies cannot be removed this way. */
export function remove(owner, id) {
  const p = proxies.get(id);
  if (!p || (p.owner !== null && p.owner !== owner)) return false;
  if (p.owner === null) throw new HttpError(Status.FORBIDDEN, 'Shared proxies are host-configured');
  for (const [persona, proxyId] of assignments) if (proxyId === id) assignments.delete(persona);
  proxies.delete(id);
  return true;
}

/** What this owner can route through: their own plus shared host proxies. */
export function visible(owner) {
  return [...proxies.values()].filter((p) => p.owner === null || p.owner === owner);
}

/** The proxies visible to an owner, credentials stripped. */
export const list = (owner) => visible(owner).map((p) => p.toJSON());

/**
 * The proxy a persona should use, assigned once and kept.
 * @returns {Proxy|null} null when no proxy is configured at all — direct egress
 *   is a valid choice, but it should be a visible one.
 */
export function forPersona(owner, persona, { geo }: any = {}) {
  const kept = keptProxy(persona);
  if (kept) return kept;
  const wanted = geo || persona.proxy?.geo || null;
  const candidates = candidatesFor(owner, wanted);
  if (!candidates.length) return null;
  const chosen = candidates.sort((a, b) => a.assigned - b.assigned)[0];
  assignments.set(persona.id, chosen.id);
  return chosen;
}

/** Available proxies with room for another persona, in the wanted geo when one is named. */
function candidatesFor(owner, wanted) {
  return visible(owner)
    .filter((p) => p.available && p.assigned < p.maxPersonas)
    .filter((p) => !wanted || p.geo === wanted || String(p.geo || '').startsWith(wanted));
}

/** The persona's existing proxy while it is available; an unavailable one is dropped. */
function keptProxy(persona) {
  const existing = assignments.get(persona.id);
  if (!existing) return null;
  const p = proxies.get(existing);
  if (p?.available) return p;
  // Its proxy is down. Reassigning changes the exit IP mid-life, which is
  // itself a signal, so say so rather than silently swapping.
  console.warn(`[proxies] ${persona.id} was on ${existing}, which is unhealthy — reassigning changes its exit IP`);
  assignments.delete(persona.id);
  return null;
}

/** Forget a persona's proxy; the next forPersona() call picks one afresh. */
export function unassign(personaId) {
  assignments.delete(personaId);
}

/** Pin a persona to one proxy. The owner check is the tenant boundary. */
export function assign(owner, personaId, proxyId) {
  const p = proxies.get(proxyId);
  if (!p || (p.owner !== null && p.owner !== owner)) return null;
  assignments.set(personaId, proxyId);
  return p;
}

/** Which proxy a persona is currently on, without assigning one. */
export function assigned(personaId) {
  const id = assignments.get(personaId);
  return id ? proxies.get(id) || null : null;
}

/**
 * Verify a proxy works and learn its exit IP, so a customer is not told an
 * identity is in Denver when its traffic leaves Frankfurt.
 */
export async function check(proxy) {
  const { url, username, password } = credentials(proxy);
  const target = new URL(process.env.OYA_PROXY_CHECK_URL || DEFAULT_CHECK_URL);
  try {
    return recordExit(proxy, await getVia(new URL(url), username, password, target));
  } catch (e) {
    proxy.fail();
    return { ok: false, error: e.message };
  }
}

/** A check answered: note the exit IP it reported and mark the proxy healthy. */
function recordExit(proxy, raw) {
  const body = JSON.parse(raw);
  proxy.succeed(body.ip || body.origin || null);
  return { ok: true, exitIp: proxy.exitIp };
}

/** Health-check every proxy the owner can see. */
export async function checkAll(owner) {
  return Promise.all(visible(owner).map(async (p) => ({ id: p.id, ...(await check(p)) })));
}

/** Test hook. */
export function reset() {
  proxies.clear();
  assignments.clear();
}
