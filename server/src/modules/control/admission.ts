/** Control-plane admission: the middleware that reserves a session before a browser is provisioned. */
import { registry } from '../browsers/registry.ts';
import { container } from '../../app/container.ts';
import * as keyConfig from '../config/service.ts';
import { Status } from '../../platform/http-status.ts';
import { BEARER_PREFIX } from './http/constants.ts';
import { reserve, answerReplay, starting, persistOutcome } from './http/admission.ts';

// Not yet layered: reads the persona service from the composition root.
const { personas } = container;

/** Persist a reservation before invoking any external provider. */
export function admission(defaultProvider?) {
  return async (req, res, next) => {
    const key = req.headers.authorization.slice(BEARER_PREFIX.length);
    try {
      await admit(key, req, res, next, defaultProvider);
    } catch (err) {
      res.status(err.status || Status.UNAVAILABLE).json({ error: err.message, code: err.code });
    }
  };
}

/** Reserves the session, then answers a replay or a queued start, or lets the start proceed. */
async function admit(key, req, res, next, defaultProvider) {
  if (registry.draining) return res.status(Status.UNAVAILABLE).json({ error: 'Server is draining', code: 'draining' });
  const { provider, persona } = chooseTarget(key, req, defaultProvider);
  const reservation = await reserve(key, req, provider, persona);
  if (reservation.replay) return answerReplay(key, req, res, reservation);
  if (reservation.state === 'queued')
    return res.status(Status.ACCEPTED).json(starting(reservation.id, provider, persona?.id, 'queued'));
  proceed(key, req, res, reservation, persona);
  next();
}

/** The provider to start on and, for a start, the persona to start as. */
function chooseTarget(key, req, defaultProvider) {
  const provider = req.body?.provider || defaultProvider || keyConfig.providerFor(key);
  const persona = req.path.endsWith('/start') ? personas.resolve(key, req.body?.profile || req.body?.persona) : null;
  return { provider, persona };
}

/** Binds the reservation to the request and persists whatever the start handler answers. */
function proceed(key, req, res, reservation, persona) {
  req.controlSession = reservation;
  if (persona) req.body = { ...req.body, persona: persona.id, profile: persona.id };
  persistOutcome(key, reservation, res);
}
