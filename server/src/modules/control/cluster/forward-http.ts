/**
 * HTTP forwarding: a request for a target that lives in another replica's
 * memory is proxied to that owner and its response streamed back.
 */
import { Readable } from 'node:stream';
import { control } from '../service.ts';
import { authenticateToken } from '../../auth/service.ts';
import { Status } from '../../../platform/http-status.ts';
import { BEARER_PREFIX } from '../http/constants.ts';
import { ownerFor } from './owner.ts';
import { hopHeader, refuseRehop, retarget, verifyHop } from './hop.ts';
import { HEADER_TIMEOUT_MS } from './constants.ts';

// Targets that live in one replica's memory: connected browsers, live streams, gateway sessions, and per-browser MCP.
const OWNED = /^\/(?:api\/(?:browsers|live|control\/sessions|gateway\/sessions)|mcp)\/([a-zA-Z0-9-]+)(?:\/|$)/;
/** Collection routes under the same prefixes, which any replica serves. */
const COLLECTIONS = ['start', 'stop', 'connect', 'provision', 'disconnect-all', 'pool'];
// MCP streamable HTTP carries its session and resumption state in headers, in both directions.
const FORWARD_HEADERS = ['accept', 'mcp-session-id', 'mcp-protocol-version', 'last-event-id'];
/** Response headers passed back from the owner. */
const RETURN_HEADERS = [
  'content-type',
  'cache-control',
  'retry-after',
  'ratelimit-limit',
  'ratelimit-remaining',
  'mcp-session-id',
];

/**
 * Express middleware: proxies a request for a replica-owned target to its owner, streaming the response back.
 * Serves locally when this replica owns it or runs alone; an already-forwarded request is never forwarded again.
 */
export async function forwardHttp(req, res, next) {
  const match = (req.baseUrl + req.path).match(OWNED);
  if (!match || COLLECTIONS.includes(match[1])) return next();
  try {
    await forwardOwned(req, res, next, match[1]);
  } catch (e) {
    answerFailure(res, e);
  }
}

/** Serves locally, or proxies to the owner of `id`. */
async function forwardOwned(req, res, next, id) {
  verifyHop(req);
  const token = await callerToken(req, id);
  // A lone replica owns everything; skip the ownership lookup.
  if (!process.env.OYA_INSTANCE_URL) return next();
  const principal = await authenticateToken(token);
  const owner = await ownerFor(id, principal.key);
  if (!owner) return next();
  refuseRehop(req, 'Session ownership changed; retry the request');
  relay(res, await fetchOwner(req, res, owner, token));
}

/** The caller's credential: header, query key, or a redeemed ticket (kept on the request). */
async function callerToken(req, id) {
  let token = req.authToken || req.headers.authorization?.slice(BEARER_PREFIX.length) || req.query.key;
  if (!token && req.query.ticket) {
    token = await control().redeem(String(req.query.ticket), id);
    req.authToken = token;
  }
  return token;
}

/** Sends the request on to the owner, signed as a hop; resolves once its response headers arrive. */
function fetchOwner(req, res, owner, token) {
  const target = retarget(owner.url, req.originalUrl || req.url, ['key', 'ticket']);
  const headers = forwardHeaders(req, token, hopHeader(req.method, target.pathname + target.search));
  // Bound the wait for the owner's response headers, not the life of a stream such as SSE.
  const abort = new AbortController(),
    headerTimeout = setTimeout(() => abort.abort(), HEADER_TIMEOUT_MS);
  res.on('close', () => abort.abort());
  return fetch(target, requestInit(req, headers, abort.signal)).finally(() => clearTimeout(headerTimeout));
}

/** Credential, hop signature and the MCP headers the owner needs. */
function forwardHeaders(req, token, hop) {
  const headers = { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json', 'X-Oya-Hop': hop };
  for (const name of FORWARD_HEADERS) if (req.headers[name]) headers[name] = req.headers[name];
  return headers;
}

/** The forwarded request: same method, JSON body unless GET or HEAD, no redirects. */
const requestInit = (req, headers, signal): RequestInit => ({
  method: req.method,
  headers,
  body: ['GET', 'HEAD'].includes(req.method) ? undefined : JSON.stringify(req.body || {}),
  redirect: 'error',
  signal,
});

/** Streams the owner's status, selected headers and body back to the caller. */
function relay(res, response) {
  res.status(response.status);
  for (const header of RETURN_HEADERS) if (response.headers.has(header)) res.set(header, response.headers.get(header));
  res.flushHeaders();
  if (response.body)
    Readable.fromWeb(response.body)
      .on('error', () => res.destroy())
      .pipe(res);
  else res.end();
}

/** A JSON error while headers can still be sent; otherwise the stream is cut. */
function answerFailure(res, e) {
  if (res.headersSent) return res.destroy();
  res.status(e.status || Status.UNAVAILABLE).json({ error: e.message, code: e.code || 'owner_unavailable' });
}
