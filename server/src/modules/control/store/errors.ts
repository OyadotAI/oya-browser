/**
 * The coded errors control storage throws. Both backends raise the same ones, so
 * callers never learn which backend answered.
 */
import { HttpError } from '../../../platform/errors.ts';
import { Status } from '../../../platform/http-status.ts';

/** An HttpError carrying an API error code. */
export const fail = (code, message, status) => new HttpError(status, message, { code });

/** A takeover while agent commands are still running. */
export const commandsPending = () =>
  fail('commands_pending', 'In-flight commands must settle before takeover', Status.CONFLICT);

/** An agent command while a human holds (or is taking) the browser. */
export const controlPaused = () =>
  fail('control_paused', 'Agent commands are paused for human takeover', Status.CONFLICT);
