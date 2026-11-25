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

/** Headers for one call: the caller's credential and an idempotency key, fresh unless the caller repeats one. */
const headersFor = (self: Self, idempotencyKey?: string) => ({
  Authorization: self.authorization,
  'Content-Type': 'application/json',
  'Idempotency-Key': idempotencyKey || globalThis.crypto.randomUUID(),
  // Names the client, so a start can be counted as one an agent asked for.
  'X-Oya-Client': 'mcp',
});

/**
 * POSTs `body` to the public API at `path` as the caller; throws its error message on failure.
 * A refusal the API answered is marked `answered`; without it the reply was lost (a timeout, a
 * dropped connection) and the call may have happened, so a caller that passes the same
 * `idempotencyKey` again gets the first outcome instead of a second start.
 */
export async function selfApi(self: Self, path: string, body?, idempotencyKey?: string) {
  if (!self.origin || !self.authorization) throw new Error('browser lifecycle is unavailable on this endpoint');
  const init = { method: 'POST', headers: headersFor(self, idempotencyKey), body: JSON.stringify(body || {}) };
  const res = await fetch(`${self.origin}/api${path}`, { ...init, signal: AbortSignal.timeout(SELF_API_TIMEOUT_MS) });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw Object.assign(new Error(data.error || `${res.status} ${res.statusText}`), { answered: true });
  return data;
}
