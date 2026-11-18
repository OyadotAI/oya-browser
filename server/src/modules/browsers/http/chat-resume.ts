/**
 * The two calls a desktop makes about a chat run that outlives its request: collect
 * the answer of a run whose connection dropped, and stop a run. Neither needs the
 * browser to be connected at that moment, because a desktop that lost its HTTP
 * request has usually lost its socket too, and both are about a run on this server.
 */
import { Status } from '../../../platform/http-status.ts';
import { sendError } from '../../../platform/errors.ts';
import { resumableRuns, runKey, type HeldRun } from '../../agent/chat.ts';
import { getKey } from '../../../app/http.ts';
import { LANE_PATTERN } from '../connection/constants.ts';
import { noTimeouts } from './helpers.ts';
import { answerWith, validToken } from './command.ts';

/** Why a body without a usable lane and resume token was refused. */
const BAD_LANE = 'lane and resume must be short names of letters, digits, dots, dashes and underscores';

/** The lane a body names, when it is a lane's name. */
const laneOf = (body): string | null =>
  typeof body?.lane === 'string' && LANE_PATTERN.test(body.lane) ? body.lane : null;

/**
 * Waits for the answer of the run this caller started in `lane` with token `resume`, as the chat
 * route would have answered it. No such run (it ended long ago, the token is another run's, or the
 * server restarted) is a 404 with code `chat_run_gone`, sent as a real status before any waiting.
 */
export async function resumeChat(req, res) {
  noTimeouts(req, res);
  const lane = laneOf(req.body);
  if (!lane || !validToken(req.body.resume)) return res.status(Status.BAD_REQUEST).json({ error: BAD_LANE });
  const run = attached(req, res, lane);
  if (run) await answerWith(res, run);
}

/** The caller's run in `lane`, or null with the refusal already answered. */
function attached(req, res, lane: string): HeldRun | null {
  try {
    return resumableRuns.attach(runKey(req.params.browserId, lane), getKey(req), req.body.resume);
  } catch (err) {
    sendError(res, err, req);
    return null;
  }
}

/** Stops this caller's chat run in `lane` named `resume` at its next step; answers whether there was one to stop. */
export function stopChat(req, res) {
  const lane = laneOf(req.body);
  if (!lane || !validToken(req.body.resume)) return res.status(Status.BAD_REQUEST).json({ error: BAD_LANE });
  const stopped = resumableRuns.stop(runKey(req.params.browserId, lane), getKey(req), req.body.resume);
  res.json({ ok: true, stopped });
}
