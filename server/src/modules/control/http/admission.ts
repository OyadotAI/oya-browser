/**
 * The steps of control-plane admission: reserve the session, answer replays
 * and queued starts, and persist the outcome of the start that goes ahead.
 */
import { control } from '../service.ts';
import { QUOTAS } from '../../../platform/limits.ts';
import { Status } from '../../../platform/http-status.ts';

/** Providers whose browsers this server runs itself. */
const MANAGED = ['oya-cloud', 'oya-selfhosted'];

/** Persists the reservation for this request. */
export function reserve(key, req, provider, persona) {
  return control().reserve(key, {
    provider,
    inheritedPolicies: req.recoveryPolicies || [],
    ...limits(persona),
    request: req.body || {},
    idempotencyKey: req.get('Idempotency-Key'),
    managed: MANAGED.includes(provider),
  });
}

/** The persona and quota limits the reservation is held to. */
const limits = (persona) => ({
  persona: persona?.id,
  personaLimit: persona?.maxConcurrent,
  maxConcurrent: QUOTAS.browsers,
  hourlyLimit: QUOTAS.sandboxesPerHour,
});

/** The 202 answer for a start that is not ready yet. */
export const starting = (id, provider, persona, state) => ({
  id,
  operationId: id,
  provider,
  persona,
  status: 'starting',
  state,
});

/** Answers a repeated request: the stored outcome (with a fresh ticket), or 202 while it is still starting. */
export async function answerReplay(key, req, res, reservation) {
  if (!reservation.response)
    return res
      .status(Status.ACCEPTED)
      .json(starting(reservation.id, reservation.provider, reservation.persona, reservation.state));
  const body = structuredClone(reservation.response.body);
  if (body.cdpUrl) body.cdpUrl = await withTicket(body.cdpUrl, key, reservation.id, req.authToken || key);
  return res.status(reservation.response.status).json(body);
}

/** The CDP URL with a fresh single-use ticket in place of any token. */
async function withTicket(cdpUrl, key, id, token) {
  const url = new URL(cdpUrl);
  url.searchParams.set('ticket', await control().ticket(key, id, token));
  url.searchParams.delete('token');
  return url.href;
}

/** Stores the handler's answer as the operation's outcome before sending it; 503 when it cannot be stored. */
export function persistOutcome(key, reservation, res) {
  const json = res.json.bind(res);
  res.json = (body) => {
    control()
      .complete(key, reservation.id, res.statusCode, body)
      .then(() => json(body))
      .catch(() => storageFailed(res, json, reservation.id));
    return res;
  };
}

/** The answer when the outcome could not be stored. */
function storageFailed(res, json, operationId) {
  res.statusCode = Status.UNAVAILABLE;
  json({ error: 'Could not persist operation outcome', code: 'storage_unavailable', operationId });
}
