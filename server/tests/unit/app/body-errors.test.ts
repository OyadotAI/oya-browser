/**
 * Unit tests for how a body the JSON parser refused is answered: in JSON, in
 * the API's one shape, with the parser's reason, and never as Express's HTML
 * page. Errors that are not the parser's pass through untouched.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { answerBodyErrors } from '../../../src/app/body-errors.ts';
import { FakeResponse } from '../support/browsers.ts';

/** A body-parser error, which names its own type. */
const parserError = (type: string, message: string, status?: number) =>
  Object.assign(new SyntaxError(message), { type, status, body: '{"name":' });

describe('answerBodyErrors', () => {
  it('answers malformed JSON as a 400 that says what the parser saw, never the raw body or a stack', () => {
    const res = new FakeResponse();
    answerBodyErrors(parserError('entity.parse.failed', 'Unexpected end of JSON input', 400), {}, res, () => {});
    assert.equal(res.statusCode, 400);
    assert.deepEqual(res.body, {
      error: 'Request body must be a JSON object: Unexpected end of JSON input.',
      code: 'invalid_request',
    });
  });

  it('keeps the 413 and its advice for a body over the limit', () => {
    const res = new FakeResponse();
    answerBodyErrors(parserError('entity.too.large', 'request entity too large', 413), {}, res, () => {});
    assert.equal(res.statusCode, 413);
    assert.match(res.body.error, /over 15MB/);
    assert.equal(res.body.code, 'invalid_request');
  });

  it('answers an unsupported charset with the parser’s own status', () => {
    const res = new FakeResponse();
    answerBodyErrors(parserError('charset.unsupported', 'unsupported charset "UTF-7"', 415), {}, res, () => {});
    assert.equal(res.statusCode, 415);
  });

  it('hands on an error that did not come from the parser', () => {
    const res = new FakeResponse();
    const err = new Error('route failed');
    let passed;
    answerBodyErrors(err, {}, res, (e) => (passed = e));
    assert.equal(passed, err);
    assert.equal(res.body, undefined);
  });
});
