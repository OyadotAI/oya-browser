/**
 * Browser lifecycle goes through the public API as the caller, so quotas,
 * budgets, persona caps, audit and billing apply exactly as over REST, and
 * a scoped credential cannot do more here than there.
 */
import { SELF_API_TIMEOUT_MS } from './constants.ts';

/** The request's own origin and Authorization header, replayed against the public API. */
export type Self = {
  /** Where this server listens, e.g. http://127.0.0.1:3100. */
  origin?: string;
  /** The caller's Authorization header. */
  authorization?: string;
};

/** Headers for one call: the caller's credential and a fresh idempotency key. */
const headersFor = (self: Self) => ({
  Authorization: self.authorization,
  'Content-Type': 'application/json',
  'Idempotency-Key': globalThis.crypto.randomUUID(),
});

/** POSTs `body` to the public API at `path` as the caller; throws its error message on failure. */
export async function selfApi(self: Self, path: string, body?) {
  if (!self.origin || !self.authorization) throw new Error('browser lifecycle is unavailable on this endpoint');
  const init = { method: 'POST', headers: headersFor(self), body: JSON.stringify(body || {}) };
  const res = await fetch(`${self.origin}/api${path}`, { ...init, signal: AbortSignal.timeout(SELF_API_TIMEOUT_MS) });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(data.error || `${res.status} ${res.statusText}`);
  return data;
}
