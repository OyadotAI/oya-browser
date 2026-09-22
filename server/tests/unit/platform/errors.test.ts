/**
 * Unit tests for the errors a route or service throws and the one answer the
 * API gives for each: a caller's mistake names the field and what it needs, a
 * missing resource says so, and an unexpected failure never shows its message,
 * only a reference that also sits in the server's log.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { HttpError, answerFor, invalid, notFound, setUnexpectedReporter } from '../../../src/platform/errors.ts';
import { Status } from '../../../src/platform/http-status.ts';

/** A request as the error handler reads it. */
const req = { method: 'POST', originalUrl: '/api/personas' };

describe('HttpError', () => {
  it('carries its status and message and is still an Error', () => {
    const err = new HttpError(Status.NOT_FOUND, 'gone missing');
    assert.ok(err instanceof Error);
    assert.equal(err.status, 404);
    assert.equal(err.message, 'gone missing');
  });

  it('copies extra fields such as a code onto the error', () => {
    const err: any = new HttpError(Status.TOO_MANY_REQUESTS, 'slow down', { code: 'rate_limited', retryAfter: 3 });
    assert.equal(err.code, 'rate_limited');
    assert.equal(err.retryAfter, 3);
  });
});

describe('invalid', () => {
  it('names the field, what it needs and the value it got, as a 400 the caller can fix', () => {
    const err: any = invalid('name', 'a string', 12345);
    assert.deepEqual([err.status, err.code, err.field], [Status.BAD_REQUEST, 'invalid_request', 'name']);
    assert.equal(err.message, 'name must be a string, not 12345');
  });

  it('quotes a short string and a boolean, and names the kind of anything else', () => {
    const got = (value) => invalid('f', 'x', value).message.replace('f must be x, not ', '');
    assert.equal(got('../x y'), '"../x y"');
    assert.equal(got(true), 'true');
    assert.equal(got({ a: 1 }), 'an object');
    assert.equal(got([1]), 'an array');
    assert.equal(got(null), 'null');
    assert.equal(got('x'.repeat(41)), 'a string');
  });
});

describe('notFound', () => {
  it('is a 404 that names what was looked for', () => {
    const err: any = notFound('Member');
    assert.deepEqual([err.status, err.code, err.message], [Status.NOT_FOUND, 'not_found', 'Member not found']);
  });
});

describe('answerFor', () => {
  afterEach(() => mock.restoreAll());

  it('answers an HttpError with its status, message, code and the fields a caller reads', () => {
    const err = new HttpError(Status.CONFLICT, 'in use', { code: 'endpoint_in_use', browserId: 'b-1', retryAfter: 2 });
    assert.deepEqual(answerFor(err, req), {
      status: Status.CONFLICT,
      body: { error: 'in use', code: 'endpoint_in_use', browserId: 'b-1', retryAfter: 2 },
    });
  });

  it('gives an HttpError without a code the code its status means', () => {
    const codeOf = (status) => answerFor(new HttpError(status, 'x'), req).body.code;
    assert.equal(codeOf(Status.BAD_REQUEST), 'invalid_request');
    assert.equal(codeOf(Status.NOT_FOUND), 'not_found');
    assert.equal(codeOf(Status.INTERNAL), 'internal_error');
    assert.equal(codeOf(Status.CONFLICT), 'operation_failed');
  });

  it('answers any other error as a 500 that hides the message and carries a reference, never a 503', () => {
    const logged = mock.method(console, 'error', () => {});
    const { status, body } = answerFor(new TypeError('name.slice is not a function'), req);
    assert.equal(status, Status.INTERNAL);
    assert.equal(body.code, 'internal_error');
    assert.match(body.ref, /^[0-9a-f]{8}$/);
    assert.equal(
      body.error,
      `Something went wrong on the server, not in the request (ref ${body.ref}). Try once more; if it happens again, report the ref.`,
    );
    assert.ok(!body.error.includes('slice'));
    const line = logged.mock.calls[0].arguments.join(' ');
    assert.ok(line.includes(`[api] 500 ref=${body.ref} POST /api/personas`), line);
    const { body: withQuery } = answerFor(new Error('x'), { method: 'GET', originalUrl: '/api/live?key=SECRET' });
    assert.ok(!logged.mock.calls.at(-1).arguments.join(' ').includes('SECRET'), 'a query string never reaches the log');
    assert.ok(withQuery.ref);
    assert.ok(line.includes('name.slice is not a function'), 'the log carries the real error');
  });

  it('tells the reporter about an unexpected failure with its ref and the route that matched, and never about an HttpError', () => {
    mock.method(console, 'error', () => {});
    const seen: any[] = [];
    setUnexpectedReporter((ref, asked) => seen.push([ref, asked.route?.path]));
    const { body } = answerFor(new Error('x'), { ...req, route: { path: '/personas/:id' } });
    answerFor(new HttpError(Status.CONFLICT, 'busy'), req);
    setUnexpectedReporter(null);
    assert.deepEqual(seen, [[body.ref, '/personas/:id']]);
  });

  it('a reporter that throws does not change the answer', () => {
    mock.method(console, 'error', () => {});
    setUnexpectedReporter(() => {
      throw new Error('reporter bug');
    });
    const { status } = answerFor(new Error('x'), req);
    setUnexpectedReporter(null);
    assert.equal(status, Status.INTERNAL);
  });

  it('answers a path Express could not decode as the caller’s mistake, not a server fault', () => {
    const logged = mock.method(console, 'error', () => {});
    const { status, body } = answerFor(new URIError('Failed to decode param'), req);
    assert.deepEqual([status, body.code], [Status.BAD_REQUEST, 'invalid_request']);
    assert.match(body.error, /percent-escapes/);
    assert.equal(logged.mock.callCount(), 0);
  });

  it('treats an error that merely has a numeric status, such as a vendor client 401, as unexpected', () => {
    mock.method(console, 'error', () => {});
    const foreign: any = new Error('Unauthorized');
    foreign.status = 401;
    foreign.code = 1;
    const { status, body } = answerFor(foreign, req);
    assert.deepEqual([status, body.code], [Status.INTERNAL, 'internal_error']);
    assert.ok(!body.error.includes('Unauthorized'));
  });
});
