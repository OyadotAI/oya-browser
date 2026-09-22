/**
 * Unit tests for scripts/worker/failure.cjs: what a failed step tells the
 * person, one message per kind of failure, each naming the target it tried.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { failureMessage, STEP_FAILED } = require('../../../../scripts/worker/failure.cjs');

const step = { candidates: [{ kind: 'css', value: '#save' }], frames: [] };
const fail = (message) => failureMessage(new Error(message), step);

describe('failureMessage', () => {
  it('says the target was not found, and which one', () => {
    assert.match(fail('Target not found. Pick a replacement or update the wait.'), /not found.*css "#save"/s);
  });

  it('says how many elements matched when more than one did', () => {
    assert.match(fail('3 elements match. Pick a unique target.'), /3 elements match/);
    assert.match(fail('strict mode violation: locator resolved to 2 elements'), /More than one element matches/);
  });

  it('says the target was hidden, and what might show it', () => {
    assert.match(
      fail('locator.click: Timeout 15000ms exceeded.\n - element is not visible'),
      /hidden.*Hover or Scroll/,
    );
  });

  it('says something covers the target', () => {
    assert.match(fail('<div class="modal"> intercepts pointer events'), /covers css "#save"/);
  });

  it('says the page was not the expected one, with the page it was', () => {
    const message =
      'expect(page).toHaveURL(expected) failed\n\nExpected: predicate to succeed\nReceived string: "https://x.test/login"';
    assert.match(fail(message), /not the expected one\. Received string: "https:\/\/x\.test\/login"/);
  });

  it('says a check failed, with what the page had', () => {
    const message = 'expect(locator).toHaveText(expected) failed\nExpected: "Saved"\nReceived: "Error"';
    assert.match(fail(message), /Check failed on css "#save"\. Received: "Error"/);
  });

  it('says a step timed out, and on what', () => {
    assert.match(fail('locator.click: Timeout 15000ms exceeded.'), /Timed out waiting for css "#save"/);
  });

  it('names the frame the target is in', () => {
    const inFrame = { ...step, frames: ['iframe[id="pay"]'] };
    assert.match(failureMessage(new Error('Target not found.'), inFrame), /in frame iframe\[id="pay"\]/);
  });

  it('keeps a stop a stop, and says the general message for anything unknown', () => {
    assert.equal(fail('Run stopped'), 'Run stopped');
    assert.equal(fail('something odd'), STEP_FAILED);
  });
});
