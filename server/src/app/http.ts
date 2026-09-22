/**
 * Helpers shared by the REST route modules.
 */

import { timingSafeEqual } from 'crypto';
import { registry } from '../modules/browsers/registry.ts';
import { sendCommand } from '../modules/browsers/socket.ts';
import { fingerprint } from '../platform/audit.ts';
import { control } from '../modules/control/service.ts';
import { Status } from '../platform/http-status.ts';
import { answerFor } from '../platform/errors.ts';
import { VOCABULARY, isInternal } from '../drivers/vocabulary.ts';
import { BEARER_PREFIX_LENGTH } from '../platform/constants.ts';
import {
  BASE64_GROUP_BYTES,
  BASE64_GROUP_CHARS,
  LONG_JSON_KEEPALIVE_MS,
  MAX_FILE_BYTES,
  MAX_FILE_NAME_CHARS,
  MAX_FILE_TYPE_CHARS,
} from './constants.ts';

/** Everything is scoped to the calling key. There is no tier above it. */
export const ownerScope = (req) => fingerprint(getKey(req));

/**
 * Process-level controls (Prometheus scrape, drain) are host operations, not
 * tenant data, so they are gated on an explicitly-named operator token rather
 * than on any API key. An API key never confers power over the host, and no
 * env var silently turns a key into a superuser.
 */
export function operatorOnly(req, res, next) {
  const token = process.env.OYA_OPERATOR_TOKEN || process.env.OYA_METRICS_TOKEN;
  // Header only: a token in the query string lands in access logs, proxy logs
  // and browser history. Prometheus sends an Authorization header natively.
  if (matchesToken(getKey(req), token)) return next();
  return res.status(Status.FORBIDDEN).json({
    error: 'Host controls need OYA_OPERATOR_TOKEN in the Authorization header',
  });
}

/** Constant-time comparison of the supplied credential with the operator token; false when either is missing. */
function matchesToken(supplied, token) {
  if (!token || !supplied) return false;
  const a = Buffer.from(supplied);
  const b = Buffer.from(token);
  return a.length === b.length && timingSafeEqual(a, b);
}

/** Extract API key from Authorization header */
export function getKey(req) {
  return req.headers.authorization?.slice(BEARER_PREFIX_LENGTH) || '';
}

/** A browser belongs to the key that connected it. Nothing else can reach it. */
export function canAccess(req, browserId) {
  return registry.belongsTo(browserId, getKey(req));
}

/**
 * Route guard for `/:browserId` paths: 404 unless that browser is connected and
 * belongs to the caller's key. 404, not 403, so another key cannot probe for ids.
 */
export function requireBrowser(req, res, next) {
  const { browserId } = req.params;
  if (!registry.isConnected(browserId) || !canAccess(req, browserId)) {
    return res.status(Status.NOT_FOUND).json({ error: `Browser ${browserId} not connected` });
  }
  next();
}

// ─── Challenges: CAPTCHA and MFA ─────────────────────────────────────────────

/** Run a script in a browser we control, whichever kind it is. */
export async function evaluateIn(browserId, expression) {
  const result = await sendCommand(browserId, 'evaluate_raw', { expression });
  return result?.data?.result ?? result?.data ?? null;
}

/**
 * Anchor, Browserbase, Steel and Browser Use solve natively; solving again pays
 * twice and can race their own attempt.
 */
export const NATIVE_CAPTCHA = ['anchor', 'browserbase', 'steel', 'browseruse'];

/** Mirrors MAX_FILE_BYTES in the SDK's file(); express.json()'s limit is sized for it. */
export { MAX_FILE_BYTES };

/** Longest base64 body a MAX_FILE_BYTES file encodes to. */
const MAX_B64_CHARS = Math.ceil(MAX_FILE_BYTES / BASE64_GROUP_BYTES) * BASE64_GROUP_CHARS;

/** A non-empty string no longer than `max`. */
const boundedString = (s, max) => typeof s === 'string' && s.length > 0 && s.length <= max;

/** A task file from the SDK's `file()`. Checked here because this is the trust boundary. */
export const validFile = (v) =>
  !!v &&
  typeof v === 'object' &&
  !Array.isArray(v) &&
  boundedString(v.file, MAX_FILE_NAME_CHARS) &&
  boundedString(v.type, MAX_FILE_TYPE_CHARS) &&
  typeof v.b64 === 'string' &&
  /^[A-Za-z0-9+/]*={0,2}$/.test(v.b64) &&
  v.b64.length <= MAX_B64_CHARS;

/**
 * Agent data and playbook variables: names to strings, numbers, or a file.
 * `secrets` passes `{ files: false }`, a file is never typed through a placeholder, so
 * redact() cannot hide one and accepting it would be a silent downgrade, not a secret.
 */
export const validData = (d, { files = true } = {}) =>
  !!d &&
  typeof d === 'object' &&
  !Array.isArray(d) &&
  Object.entries(d).every(
    ([k, v]) => /^\w{1,64}$/.test(k) && (['string', 'number'].includes(typeof v) || (files && validFile(v))),
  );

/** A control-plane event for this key's webhook; never fails the caller. */
export const announce = (key, type, sessionId, detail) =>
  control()
    .emit(key, type, sessionId, detail)
    .catch(() => {});

// Send command to a browser
/**
 * Actions the server sends on its own behalf and a caller may not.
 *
 * `evaluate_raw` runs arbitrary JavaScript in the page's own world, which is
 * how CAPTCHA and MFA handling reach a site's globals. Exposed here it would
 * be arbitrary code execution inside a browser holding the customer's real
 * cookies and logged-in sessions. Neither driver refuses it by name: both run
 * it for the server. `refuseAction` on the two command routes is the refusal,
 * and it takes a string only, since `["evaluate_raw"]` is not in this set yet
 * reads as the same key in a command map.
 */
export const INTERNAL_ACTIONS = new Set(Object.keys(VOCABULARY).filter(isInternal));

/**
 * Agent work can outlast Node fetch's 300s headers timeout (and proxy idle
 * timeouts), so commit to 200 now and trickle whitespace; JSON.parse ignores it.
 * Errors after this point can only travel in the body.
 */
export async function longJson(res, work) {
  res.writeHead(Status.OK, { 'Content-Type': 'application/json' });
  const keepalive = setInterval(() => res.write(' '), LONG_JSON_KEEPALIVE_MS);
  try {
    await endWithResult(res, work);
  } finally {
    clearInterval(keepalive);
  }
}

/** End the response with the work's result, or with its error once the 200 is out. */
async function endWithResult(res, work) {
  try {
    res.end(JSON.stringify(await work()));
  } catch (err) {
    // The 200 is already sent, so the real status (429 for a quota) travels in the body, in the API's one shape.
    const { status, body } = answerFor(err, res.req ?? {});
    res.end(JSON.stringify({ ...body, status }));
  }
}
