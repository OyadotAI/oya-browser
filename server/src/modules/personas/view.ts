/**
 * The public view of a persona: what the API and dashboard show. The
 * fingerprint is described, never the seed, and a proxy's credentials never
 * leave the server.
 */
import { publicPrefs, describeProfile, DEFAULT_MAX_CONCURRENT, type Persona, type PersonaProxy } from './model.ts';
import type { PersonaDeps, PersonaService } from './service.ts';

/** Public view: fingerprint included, seed and raw proxy credentials not. */
export function describePersona(p: Persona, personas: PersonaService, deps: PersonaDeps) {
  return {
    ...summaryOf(p, personas),
    // The proxy it is actually on, when one has been assigned or pinned.
    exit: exitOf(p, deps),
    prefs: publicPrefs(p.prefs),
    fingerprint: describeProfile(personas.fingerprintFor(p)),
    ...signInOf(p.id, deps),
  };
}

/** Identity, usage and the persona's own proxy, without its credentials. */
function summaryOf(p: Persona, personas: PersonaService) {
  return {
    id: p.id,
    name: p.name,
    isDefault: !!p.isDefault,
    createdAt: p.createdAt,
    lastUsedAt: p.lastUsedAt || null,
    ...usageOf(p, personas),
  };
}

/** Browsers running now, the cap, and where the persona's own proxy points. */
const usageOf = (p: Persona, personas: PersonaService) => ({
  activeBrowsers: personas.activeCount(p.id),
  maxConcurrent: p.maxConcurrent ?? DEFAULT_MAX_CONCURRENT,
  proxy: proxyOf(p.proxy),
});

/** Host, port and geo of the persona's own proxy; never its credentials. */
const proxyOf = (proxy: PersonaProxy | null) =>
  proxy ? { host: proxy.host, port: proxy.port, geo: proxy.geo || null } : null;

/** The proxy the persona is assigned or pinned to, else the residential one it would fall back to. */
function exitOf(p: Persona, deps: PersonaDeps) {
  const exit = deps.proxies.assigned(p.id);
  return exit ? { id: exit.id, label: exit.label, geo: exit.geo, healthy: exit.available } : residentialExit(p, deps);
}

/** The operator's residential gateway, when this persona would fall back to it. */
function residentialExit(p: Persona, deps: PersonaDeps) {
  if (p.proxy?.host) return null;
  const r = deps.proxies.residential(p);
  return r ? { id: 'residential', label: 'Oya residential', geo: r.geo, healthy: true } : null;
}

/** What the persona can sign in with: its factors, site credentials and login state. */
function signInOf(id: string, deps: PersonaDeps) {
  return {
    mfa: deps.mfa.describe(id),
    // Per-site factors and credentials, so the dashboard can show which portals
    // this persona can sign in to on its own. Usernames and types only.
    sites: { mfa: deps.mfa.list(id), credentials: deps.credentials.list(id) },
    login: deps.logins.summary(id),
  };
}
