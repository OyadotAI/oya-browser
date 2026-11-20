/**
 * Mounts the SDK's OAuth endpoints (/authorize, /token, /register, /revoke and the
 * two .well-known documents) at the server's public origin, and tells a 401 from
 * the pool where to start signing in. OAuth needs an HTTPS origin (or localhost):
 * a server reachable only over plain HTTP elsewhere keeps API keys alone.
 */
import { Router } from 'express';
import { rateLimit } from 'express-rate-limit';
import { mcpAuthRouter, getOAuthProtectedResourceMetadataUrl } from '@modelcontextprotocol/sdk/server/auth/router.js';
import { clientAddress } from '../../modules/auth/service.ts';
import { consoleUrl } from '../../modules/slack/service.ts';
import { Status } from '../../platform/http-status.ts';
import {
  AUTHORIZE_REQUIRED,
  CONNECT_PAGE,
  REGISTRATIONS_PER_HOUR,
  REGISTRATION_WINDOW_MS,
  RESOURCE_NAME,
  RESOURCE_PATH,
} from './constants.ts';
import { provider } from './provider.ts';

/**
 * A backstop over every registration: the SDK's per-address limit (20 an hour,
 * keyed on clientAddress) is the real control, and this bounds how fast the
 * store can fill if that key is wrong. Callers that reach the load balancer
 * without Cloudflare can still name their own address, so one of them can use
 * this up; trusting Cloudflare's header only from Cloudflare's ranges closes that.
 */
const registrations = rateLimit({
  windowMs: REGISTRATION_WINDOW_MS,
  limit: REGISTRATIONS_PER_HOUR,
  keyGenerator: () => 'all',
  validate: false,
  standardHeaders: true,
  legacyHeaders: false,
});

/**
 * /authorize must name a code flow with PKCE before anything happens: the SDK
 * answers a malformed request with a redirect to the client's registered URI,
 * which would let anyone registered turn this origin into a redirector.
 */
function wellFormedAuthorize(req, res, next) {
  onlyToConsole(res);
  const missing = Object.entries(AUTHORIZE_REQUIRED).find(([name, value]) => req.query[name] !== value);
  if (!missing && req.query.code_challenge) return next();
  res
    .status(Status.BAD_REQUEST)
    .json({ error: 'invalid_request', error_description: 'response_type=code with S256 PKCE is required' });
}

/**
 * /authorize may only send the browser to this server's own connect page. The SDK
 * answers some refusals (a scope, a resource) with a redirect to the client's
 * registered address, and anyone can register a client, so those become a plain
 * 400 here instead: this origin never forwards a browser to an address it did not choose.
 */
function onlyToConsole(res) {
  const redirect = res.redirect.bind(res);
  res.redirect = (status: number, url: string) => {
    if (typeof url === 'string' && url.startsWith(`${consoleUrl()}${CONNECT_PAGE}?`)) return redirect(status, url);
    res
      .status(Status.BAD_REQUEST)
      .json({ error: 'invalid_request', error_description: 'This sign-in request was refused' });
  };
}

/** A body the SDK's JSON parser refused answers as OAuth does, not as Express's HTML page. */
function badJson(err, req, res, next) {
  if (err?.type !== 'entity.parse.failed') return next(err);
  res.status(Status.BAD_REQUEST).json({ error: 'invalid_request', error_description: 'Body is not valid JSON' });
}

/** Per caller address, past the ingress, so everyone behind one load balancer does not share a bucket. */
const limited = { rateLimit: { keyGenerator: clientAddress, validate: false } };
/** Every endpoint the SDK rate-limits gets the same per-address limit. */
const LIMITS = {
  authorizationOptions: limited,
  tokenOptions: limited,
  revocationOptions: limited,
  clientRegistrationOptions: limited,
};

/** The protected resource's metadata URL once OAuth is mounted, else ''. */
let metadataUrl = '';

/** The OAuth endpoints, or null when the public origin cannot be an OAuth issuer. */
export function oauthRouter() {
  try {
    return mount(new URL(consoleUrl()));
  } catch (err) {
    console.warn(`[mcp] OAuth sign-in is off: ${err.message}`);
    return null;
  }
}

/** The SDK's router for this issuer; throws when the issuer is not HTTPS or localhost. */
function mount(issuerUrl: URL) {
  const resourceServerUrl = new URL(RESOURCE_PATH, issuerUrl);
  const sdk = mcpAuthRouter({ provider, issuerUrl, resourceServerUrl, resourceName: RESOURCE_NAME, ...LIMITS });
  metadataUrl = getOAuthProtectedResourceMetadataUrl(resourceServerUrl);
  return Router().use('/register', registrations).get('/authorize', wellFormedAuthorize).use(sdk).use(badJson);
}

/** The WWW-Authenticate value a 401 from the pool carries, or '' when OAuth is off. */
export const challenge = () => (metadataUrl ? `Bearer resource_metadata="${metadataUrl}"` : '');
