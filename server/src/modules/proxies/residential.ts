/**
 * A residential exit out of the box, from one vendor gateway the operator pays
 * for (OYA_RESIDENTIAL_PROXY_URL). Used only when a persona has no proxy of its
 * own. `{session}` becomes a sticky id derived from the persona, so an identity
 * keeps its exit IP across connects; `{geo}` becomes its two-letter country.
 * Vendors spell these inside the username, e.g.
 *   http://user-country-{geo}-session-{session}:pass@gate.vendor.com:7000
 * ponytail: sticky for as long as the vendor holds a session (often 30 min to
 * 24 h); a per-persona vendor sub-user is the upgrade if IPs must never move.
 */
import { createHash } from 'crypto';
import { COUNTRY_CODE_CHARS, DEFAULT_RESIDENTIAL_GEO, SESSION_ID_CHARS } from './constants.ts';

/** The persona's residential exit, or null when the operator has none configured. */
export function residential(persona, env = process.env) {
  const template = env.OYA_RESIDENTIAL_PROXY_URL;
  if (!template || !persona?.id) return null;
  const geo = String(persona.proxy?.geo || env.OYA_RESIDENTIAL_PROXY_GEO || DEFAULT_RESIDENTIAL_GEO)
    .slice(0, COUNTRY_CODE_CHARS)
    .toLowerCase();
  const session = createHash('sha256').update(`residential:${persona.id}`).digest('hex').slice(0, SESSION_ID_CHARS);
  return exitFrom(new URL(template.replaceAll('{session}', session).replaceAll('{geo}', geo)), geo);
}

/** The filled-in gateway URL split into address and credentials. */
function exitFrom(u, geo) {
  return {
    url: `${u.protocol}//${u.host}`,
    username: decodeURIComponent(u.username) || null,
    password: decodeURIComponent(u.password) || null,
    geo: geo.toUpperCase(),
  };
}
