/**
 * The three Oya API calls the extension makes, each with the person's key as
 * a bearer token and only ever to the hosted service.
 */
import { NO_RESPONSE, OYA_ORIGIN } from './constants.js';

/** A refused or failed call: the HTTP status (0 when Oya was never reached) and the server's reason. */
export class OyaError extends Error {
  /** Builds the error from the status and the server's own error text. */
  constructor(status, reason) {
    super(reason || `Oya answered ${status}`);
    /** The HTTP status, or 0 when the request did not reach Oya. */
    this.status = status;
    /** The server's error text, empty when it gave none. */
    this.reason = reason || '';
  }
}

/** The request options for one call: the key as a bearer token, and a JSON body when there is one. */
function requestInit(key, method, body) {
  const headers = { Authorization: `Bearer ${key}` };
  if (body === undefined) return { method, headers };
  return { method, headers: { ...headers, 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
}

/** Sends one call and returns its JSON, or throws OyaError with the status and the server's reason. */
async function call(fetchFn, key, method, path, body) {
  const res = await fetchFn(`${OYA_ORIGIN}${path}`, requestInit(key, method, body)).catch(() => null);
  if (!res) throw new OyaError(NO_RESPONSE, '');
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new OyaError(res.status, data.error);
  return data;
}

/**
 * The client for one key. `fetchFn` is the browser's fetch, injected so tests
 * can answer for the server.
 */
export function oyaClient(key, fetchFn = globalThis.fetch) {
  const send = (method, path, body) => call(fetchFn, key, method, path, body);
  return {
    /** The key's role and project, so a viewer key is refused before any cookie is read. */
    whoami: () => send('GET', '/api/auth/whoami'),
    /** The key's profiles (personas): `[{ id, name }]`. */
    listProfiles: async () => (await send('GET', '/api/personas')).personas ?? [],
    /** Merges cookies into a profile's jar; answers `{ imported, skipped, total }`. */
    sendCookies: (profileId, cookies) =>
      send('PUT', `/api/pool/cookies?persona=${encodeURIComponent(profileId)}`, { cookies }),
  };
}
