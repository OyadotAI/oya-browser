/** Credentials for external Oya compatibility connections; never place bearer secrets in endpoint URLs. */
import { MAX_BEARER_TOKEN_LENGTH } from './constants.ts';

/** Explicit credentials kept separate from serializable provider configuration. */
export interface ConnectionCredentials {
  /** A short-lived front-door bearer token, not a browser profile or account credential. */
  bearerToken?: string;
}

/** Validate credentials before dialing; plaintext bearer transport is allowed only on literal loopback. */
export function connectionHeaders(url: string, credentials: ConnectionCredentials): Record<string, string> {
  const token = credentials.bearerToken;
  if (token === undefined) return {};
  if (typeof token !== 'string' || token.length > MAX_BEARER_TOKEN_LENGTH || !/^[A-Za-z0-9._~+/-]+=*$/.test(token))
    throw new Error('Invalid front-door bearer token');
  requireSecureEndpoint(url);
  return { Authorization: `Bearer ${token}` };
}

/** Reject embedded credentials and network-cleartext destinations without echoing secret input. */
function requireSecureEndpoint(url: string) {
  const endpoint = parseEndpoint(url);
  const loopback = ['127.0.0.1', '[::1]'].includes(endpoint.hostname);
  if (endpoint.username || endpoint.password || endpoint.search || endpoint.hash)
    throw new Error('Authenticated front-door endpoint must not contain credentials, query or fragment');
  if (endpoint.protocol !== 'wss:' && !(endpoint.protocol === 'ws:' && loopback))
    throw new Error('Front-door bearer authentication requires WSS or literal loopback WS');
}

/** Parsing failures must not echo an endpoint that could contain a secret. */
function parseEndpoint(url: string) {
  try {
    return new URL(url);
  } catch {
    throw new Error('Invalid authenticated front-door endpoint');
  }
}
