/** Control-plane admission: the middleware that reserves a session before a browser is provisioned. */
import { registry } from '../browsers/registry.ts';
import { container } from '../../app/container.ts';
import * as keyConfig from '../config/service.ts';
import { Status } from '../../platform/http-status.ts';
import { BEARER_PREFIX } from './http/constants.ts';
import { reserve, answerReplay, starting, persistOutcome } from './http/admission.ts';
import { startAs } from './http/start-as.ts';

// Not yet layered: reads the persona and group services from the composition root.
const { personas, groups } = container;

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

/**
 * Reserves the session, then answers a replay or a queued start, or lets the
 * start proceed. The persona the session runs as is read back from the
 * reservation: for a group start the reservation is where it was chosen.
 */
async function admit(key, req, res, next, defaultProvider) {
  if (registry.draining) return res.status(Status.UNAVAILABLE).json({ error: 'Server is draining', code: 'draining' });
  const { provider, target } = await chooseTarget(key, req, defaultProvider);
  const reservation = await reserve(key, req, provider, target);
  if (reservation.replay) return answerReplay(key, req, res, reservation);
  if (reservation.state === 'queued')
    return res.status(Status.ACCEPTED).json(starting(reservation.id, provider, reservation.persona, 'queued'));
  proceed(key, req, res, reservation);
  next();
}

/** The provider to start on and, for a start, what it runs as: a persona, or a group's members. */
async function chooseTarget(key, req, defaultProvider) {
  const provider = req.body?.provider || defaultProvider || keyConfig.providerFor(key);
  const target = req.path.endsWith('/start') ? await startAs({ personas, groups }, key, req.body) : {};
  return { provider, target };
}

/** Binds the reservation to the request, as the persona it runs as, and persists whatever the start handler answers. */
function proceed(key, req, res, reservation) {
  req.controlSession = reservation;
  if (reservation.persona) req.body = { ...req.body, persona: reservation.persona, profile: reservation.persona };
  persistOutcome(key, reservation, res);
}
