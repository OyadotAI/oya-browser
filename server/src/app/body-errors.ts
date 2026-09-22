/**
 * What a request whose body could not be read is told. Express's JSON parser
 * throws before any route runs, and left alone its error reaches Express's own
 * HTML page, a stack trace with server paths on it. A caller of a JSON API is
 * answered in JSON, in the API's one shape, with the parser's reason.
 */
import { Status } from '../platform/http-status.ts';
import { HttpError, sendError } from '../platform/errors.ts';

/** What an oversize body is told: the limit is sized for task files, so the way out is fewer or smaller ones. */
const TOO_LARGE = 'This request is over 15MB. Task files ride inline, so send fewer or smaller ones in one run.';

/** Whether the error came from the body parser: it names its own type, a route's error does not. */
const fromParser = (err) => typeof err?.type === 'string';

/** The error a body-parser failure is answered with: a 413 in its own words, anything else as a bad request. */
function bodyError(err): HttpError {
  if (err.type === 'entity.too.large')
    return new HttpError(Status.PAYLOAD_TOO_LARGE, TOO_LARGE, { code: 'invalid_request' });
  const status = typeof err.status === 'number' ? err.status : Status.BAD_REQUEST;
  return new HttpError(status, `Request body must be a JSON object: ${err.message}.`, { code: 'invalid_request' });
}

/** Middleware: answers a body the parser refused; hands every other error on. */
export function answerBodyErrors(err, req, res, next) {
  if (!fromParser(err)) return next(err);
  sendError(res, bodyError(err), req);
}
