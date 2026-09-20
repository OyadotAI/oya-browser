/**
 * Pool route handlers: a round-robin command, and clearing a persona's
 * cookie jar.
 */
import { sendCommand } from '../socket.ts';
import { nextBrowser } from '../pool.ts';
import { clear as clearCookies } from '../../personas/cookies.ts';
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
