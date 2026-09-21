/**
 * The OAuth `state`: a single-use value kept in the control store, with its
 * other half in a cookie on the browser that started the install.
 */
import { randomBytes } from 'node:crypto';
import { control, projectId, keyOfProject } from '../control/service.ts';
import { STATE_TTL_MS, STATE_BYTES } from './constants.ts';
import { sameText } from './signature.ts';

/**
 * The half of the state that lives in the browser that started the install, so a
 * state handed to someone else is worthless. SameSite=Lax still rides Slack's
 * top-level redirect back here; the path confines it to these two routes.
 * ponytail: no cookie parser in this server, and one name is all that is read.
 */
const STATE_COOKIE = 'oya_slack_state';
/** The one cookie path the state cookie is sent on. */
const COOKIE_PATH = '/api/slack';

/** One cookie's value from the request, or undefined. */
const readCookie = (req, name) =>
  (req.headers.cookie || '')
    .split(';')
    .map((c) => c.trim())
    .find((c) => c.startsWith(`${name}=`))
    ?.slice(name.length + 1);

/** Drop the state in a short-lived, HTTP-only cookie scoped to /api/slack. */
export function setStateCookie(req, res, state) {
  const secure = req.secure || String(req.headers['x-forwarded-proto'] || '').includes('https');
  res.cookie(STATE_COOKIE, state, { httpOnly: true, secure, sameSite: 'lax', maxAge: STATE_TTL_MS, path: COOKIE_PATH });
}
/** Remove the state cookie; the callback does this whatever happens next. */
export const clearStateCookie = (res) => res.clearCookie(STATE_COOKIE, { path: COOKIE_PATH });

/** Constant-time, and false for an absent cookie rather than matching an absent state. */
export function matchesStateCookie(req, state) {
  const cookie = readCookie(req, STATE_COOKIE);
  if (!cookie || !state) return false;
  return sameText(cookie, state);
}

/**
 * A single-use, five-minute state, kept in the control store rather than in memory
 * so the callback may land on any replica. `expires_at` puts its cleanup on the
 * maintenance sweep that already prunes expired rows.
 */
export async function issueState(key) {
  const state = randomBytes(STATE_BYTES).toString('base64url');
  await control().store.transact(async (tx) => {
    // The project id, never the key: the project row already holds the key sealed,
    // so this adds no second copy of a credential at rest.
    tx.put('slack_state', state, { id: state, project: projectId(key), expiresAt: Date.now() + STATE_TTL_MS });
  });
  return state;
}

/** Take the state row out of the store; its project if it had not expired. */
async function consume(tx, state) {
  const row = await tx.get('slack_state', state);
  if (!row?.project) return null;
  await tx.delete('slack_state', state);
  return row.expiresAt > Date.now() ? row.project : null;
}

/** The state's project key, once, or null. Consumed whether or not it had expired. */
export async function redeemState(state) {
  if (!state) return null;
  const project = await control().store.transact((tx) => consume(tx, state));
  return project ? keyOfProject(project) : null;
}
