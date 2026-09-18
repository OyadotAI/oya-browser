/**
 * Unit tests for HttpError: an error that carries the status it is answered
 * with, plus any extra fields callers branch on.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { HttpError } from '../../../src/platform/errors.ts';
import { Status } from '../../../src/platform/http-status.ts';

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
