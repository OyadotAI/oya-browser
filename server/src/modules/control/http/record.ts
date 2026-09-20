/**
 * POST /control/sessions/:id/record: start, stop, discard or poll recording a
 * flow the person demonstrates. It runs under the same takeover as their clicks,
 * so the poll is not an agent command competing with them for the browser.
 */
import { control, hash, fault } from '../service.ts';
import { sendCommand } from '../../browsers/socket.ts';
import * as flow from '../../playbooks/flow-recorder.ts';
import { Status } from '../../../platform/http-status.ts';

/** What one recording request works with. */
type Recording = {
  /** The caller's project key. */
  key: string;
  /** The browser being recorded. */
  id: string;
  /** The person's control-holder identity. */
  holder: string;
  /** The request body. */
  body: any;
  /** Sends a command as the person. */
  dispatch: (action, params) => Promise<any>;
};

/** Each recording mode. */
const MODES: Record<string, (r: Recording) => Promise<unknown>> = {
  start: (r) => flow.start(r.id, r.dispatch),
  stop: (r) => stop(r),
  discard: (r) => flow.discard(r.id),
  status: (r) => flow.status(r.id, r.dispatch),
};

/** Runs the requested mode and returns what the recorder reports. */
export async function recordFlow(key, req) {
  const holder = hash(req.authToken),
    id = req.params.id;
  const dispatch = (action, params) => sendCommand(id, action, params, undefined, holder);
  const mode = req.body?.mode;
  if (!Object.hasOwn(MODES, mode))
    throw fault('invalid_mode', 'mode must be start, stop, status or discard', Status.BAD_REQUEST);
  return MODES[mode]({ key, id, holder, body: req.body, dispatch });
}

/** Stops recording and, with `resume: true`, hands the browser back to the agent. */
async function stop(r: Recording) {
  const final = (await flow.stop(r.id, r.dispatch)) || (await flow.status(r.id));
  if (r.body?.resume === true) {
    await control().takeover(r.key, r.id, 'release', r.holder);
    await control().takeover(r.key, r.id, 'resume', r.holder);
  }
  return final;
}
