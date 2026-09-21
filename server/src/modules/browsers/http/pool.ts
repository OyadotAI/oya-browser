/**
 * Pool route handlers: a round-robin command, and a persona's cookie jar:
 * exporting it, importing into it, and clearing it.
 */
import { sendCommand } from '../socket.ts';
import { nextBrowser } from '../pool.ts';
import { clear as clearCookies, getAll as getAllCookies, mergeDump } from '../../personas/cookies.ts';
import { FORMATS, formatJar } from '../../personas/cookie-formats.ts';
import { validCookie, expired } from '../../personas/cookie-rules.ts';
import { MAX_IMPORT_COOKIES } from '../constants.ts';
import { audit } from '../../../platform/audit.ts';
import { Status } from '../../../platform/http-status.ts';
import { getKey } from '../../../app/http.ts';
import { resolvePersona } from '../lifecycle/persona.ts';
import { noTimeouts, refuseAction } from './helpers.ts';

/** Sends a command to the next browser in the caller's pool, round-robin. */
export async function poolCommand(req, res) {
  noTimeouts(req, res);
  const key = getKey(req);
  const { action, params } = req.body;
  // The pool is the same surface by another door.
  if (refuseAction(res, action)) return;
  const browserId = nextBrowser(key);
  if (!browserId) return res.status(Status.UNAVAILABLE).json({ error: 'No browsers available in pool' });
  await dispatch(res, browserId, action, params);
}

/** Runs the command and answers with the browser that ran it. */
async function dispatch(res, browserId, action, params) {
  try {
    const result = await sendCommand(browserId, action, params || {});
    res.json({ ...result, _browser: browserId });
  } catch (err) {
    res.status(Status.INTERNAL).json({ ok: false, error: err.message, _browser: browserId });
  }
}

/** ?persona= as one id. A repeated param becomes an unknown id (404), never the default persona. */
export const queryPersona = (req) => (req.query.persona === undefined ? undefined : String(req.query.persona));

/** Clears one of the caller's persona jars (?persona=, else the default); audited. */
export function clearJar(req, res) {
  const key = getKey(req);
  const resolved = resolvePersona(res, key, queryPersona(req), Status.INTERNAL);
  if (!resolved) return;
  const { persona } = resolved;
  clearCookies(persona.id);
  // Destroying sessions is exactly the action you want a record of afterwards.
  audit({ action: 'cookies.clear', actorKey: key, targetType: 'cookies', targetId: persona.id, req });
  res.json({ ok: true, persona: persona.id });
}

/**
 * Answers one of the caller's persona jars (?persona=, else the default) in ?format=:
 * json as stored, playwright for `context.addCookies`, or netscape, a cookies.txt download.
 */
export function exportJar(req, res) {
  const resolved = resolvePersona(res, getKey(req), queryPersona(req), Status.INTERNAL);
  if (!resolved) return;
  const format = req.query.format === undefined ? 'json' : String(req.query.format);
  const jar = formatJar(format, getAllCookies(resolved.persona.id));
  if (jar === undefined)
    return res.status(Status.BAD_REQUEST).json({ error: `format must be one of: ${FORMATS.join(', ')}` });
  if (typeof jar === 'string') return sendCookiesTxt(res, resolved.persona.id, jar);
  res.json({ persona: resolved.persona.id, cookies: jar });
}

/** Sends a cookies.txt as a file to save. */
function sendCookiesTxt(res, personaId, text) {
  res.set('Content-Type', 'text/plain; charset=utf-8');
  res.set('Content-Disposition', `attachment; filename="cookies-${personaId}.txt"`);
  res.end(text);
}

/** Why this body cannot be imported, or null when it carries a bounded list of cookies. */
function importRefusal(body) {
  if (!Array.isArray(body?.cookies)) return 'Send { "cookies": [...] }: a list of cookies with name, value and domain';
  return body.cookies.length > MAX_IMPORT_COOKIES ? `At most ${MAX_IMPORT_COOKIES} cookies per import` : null;
}

/**
 * Merges cookies into one of the caller's persona jars (?persona=, else the default); audited.
 * The jar's browsers pick them up on their next visit to each site. A cookie
 * without a name, value or domain, or one already expired, is skipped and counted.
 */
export function importJar(req, res) {
  const key = getKey(req);
  const resolved = resolvePersona(res, key, queryPersona(req), Status.INTERNAL);
  if (!resolved) return;
  const refusal = importRefusal(req.body);
  if (refusal) return res.status(Status.BAD_REQUEST).json({ error: refusal });
  res.json({ ok: true, persona: resolved.persona.id, ...mergeInto(resolved.persona.id, req.body.cookies, key, req) });
}

/** Merges the usable cookies, records who did it, and counts what happened. */
function mergeInto(personaId, cookies, key, req) {
  const usable = cookies.filter((c) => validCookie(c) && !expired(c));
  const total = mergeDump(personaId, usable).length;
  // Planting sessions in a jar is exactly the action you want a record of afterwards.
  audit({ action: 'cookies.import', actorKey: key, targetType: 'cookies', targetId: personaId, req });
  return { imported: usable.length, skipped: cookies.length - usable.length, total };
}
