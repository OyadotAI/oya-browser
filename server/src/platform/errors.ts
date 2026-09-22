/**
 * Errors that carry their HTTP answer, and the one way every error is answered.
 * A route or service throws an HttpError and the API's error handlers turn it
 * into the response, so no handler needs its own try/catch just to translate.
 * Anything else that is thrown is a bug: it is answered with a generic body and
 * a reference, and logged in full under that reference.
 */
import { randomUUID } from 'node:crypto';
import { Status } from './http-status.ts';
import { GOT_MAX_CHARS, REF_CHARS } from './constants.ts';

/** An error with the status (and optional code or detail) it should be answered with. */
export class HttpError extends Error {
  /** HTTP status to answer with. */
  declare status: number;
  /** Machine-readable reason, when a caller branches on it. */
  declare code?: string;

  /** `extra` carries any further fields callers read (code, active, retryAfter…). */
  constructor(status: number, message: string, extra: Record<string, unknown> = {}) {
    super(message);
    this.status = status;
    Object.assign(this, extra);
  }
}

/** A value as a message names it: quoted when short and simple, else by its kind, so a caller sees what they sent. */
function describe(got: unknown): string {
  if (got === null) return 'null';
  if (Array.isArray(got)) return 'an array';
  if (typeof got === 'object') return 'an object';
  if (typeof got === 'string' && got.length > GOT_MAX_CHARS) return 'a string';
  return JSON.stringify(got);
}

/** A caller's mistake in one field, said the same way everywhere: what the field needs, and what it got. */
export const invalid = (field: string, needs: string, got: unknown) =>
  new HttpError(Status.BAD_REQUEST, `${field} must be ${needs}, not ${describe(got)}`, {
    code: 'invalid_request',
    field,
  });

/** The resource named in the path is not there. */
export const notFound = (what: string) => new HttpError(Status.NOT_FOUND, `${what} not found`, { code: 'not_found' });

/** The code an HttpError means when it names none. */
const DEFAULT_CODES: Record<number, string> = {
  [Status.BAD_REQUEST]: 'invalid_request',
  [Status.NOT_FOUND]: 'not_found',
  [Status.INTERNAL]: 'internal_error',
};

/** What an unexpected failure tells the caller: not their fault, and how to report it. */
const unexpected = (ref: string) =>
  `Something went wrong on the server, not in the request (ref ${ref}). Try once more; if it happens again, report the ref.`;

/** The part of a request a log line names, so a reference can be matched to what was asked. */
export type Asked = {
  /** The HTTP method. */
  method?: string;
  /** The path as requested. */
  originalUrl?: string;
  /** The route that matched, when one did. */
  route?: {
    /** Its path template, such as /browsers/:id/command. */
    path?: string;
  };
  /** The request's headers, for whoever reports the failure to say who hit it. */
  headers?: Record<string, unknown>;
};

/** Told about every unexpected failure once its reference is minted; the reporter must never throw or slow the answer. */
let reporter: ((ref: string, req: Asked) => void) | null = null;

/** Installs the one reporter for unexpected failures, from the composition root. */
export function setUnexpectedReporter(fn: ((ref: string, req: Asked) => void) | null) {
  reporter = fn;
}

/** Tells the reporter, if any; a reporter that throws is a bug in the reporter, not a second failure for the caller. */
function report(ref: string, req: Asked) {
  try {
    reporter?.(ref, req);
  } catch {}
}

/** The status and JSON body an error is answered with. */
export type Answer = {
  /** HTTP status. */
  status: number;
  /** The body: `error`, `code`, and any fields the caller reads. */
  body: Record<string, unknown>;
};

/** The answer for an HttpError: its own message, its code or its status's, and the fields it carries. */
function answerForHttp(err: HttpError): Answer {
  const { status, message, ...fields } = err as HttpError & Record<string, unknown>;
  const code = err.code ?? DEFAULT_CODES[status] ?? 'operation_failed';
  return { status, body: { ...fields, error: message, code } };
}

/**
 * The answer for anything else: a generic 500 under a reference, with the real
 * error logged once under that reference. The message is never exposed, because
 * it is not about the request: a vendor client's own 401 must not read as ours.
 */
function answerForUnexpected(err: unknown, req: Asked): Answer {
  const ref = randomUUID().replace(/-/g, '').slice(0, REF_CHARS);
  const stack = err instanceof Error ? err.stack || err.message : String(err);
  // The path only: a query string can carry a key or a ticket, and a log is not where those belong.
  console.error(`[api] 500 ref=${ref} ${req.method} ${String(req.originalUrl).split('?')[0]}`, stack);
  report(ref, req);
  return { status: Status.INTERNAL, body: { error: unexpected(ref), code: 'internal_error', ref } };
}

/** A path Express could not decode: a malformed percent-escape is the caller's, and Express's own URIError says so. */
const badPath = () =>
  new HttpError(Status.BAD_REQUEST, 'The request path could not be decoded. Check its percent-escapes.', {
    code: 'invalid_request',
  });

/** How any thrown error is answered. Only an HttpError speaks for itself; a URIError is a bad path, not a bug. */
export function answerFor(err: unknown, req: Asked): Answer {
  if (err instanceof URIError) return answerForHttp(badPath());
  return err instanceof HttpError ? answerForHttp(err) : answerForUnexpected(err, req);
}

/** Writes an error's answer to `res`, for validators that answer directly. */
export function sendError(res, err: unknown, req = res.req ?? {}) {
  const { status, body } = answerFor(err, req);
  return res.status(status).json(body);
}
