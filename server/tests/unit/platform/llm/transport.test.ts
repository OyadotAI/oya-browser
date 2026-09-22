/**
 * Unit tests for platform/llm/transport.ts: which failures are tried again, how
 * long each retry waits, and that a request that never answers becomes the same
 * error as any other.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  LlmError,
  backoff,
  retryAfter,
  retryable,
  unanswered,
  withRetries,
} from '../../../../src/platform/llm/transport.ts';
import { LLM_ATTEMPTS, LLM_RETRY_MAX_MS } from '../../../../src/platform/constants.ts';

/** An attempt that fails with each of `errors` in turn, then answers 'ok'. */
function flaky(errors: LlmError[]) {
  let calls = 0;
  const attempt = async () => {
    calls++;
    if (errors.length) throw errors.shift();
    return 'ok';
  };
  return { attempt, calls: () => calls };
}

/** A wait that returns at once and remembers how long it was asked to wait. */
function waits() {
  const asked: number[] = [];
  return { wait: async (ms: number) => void asked.push(ms), asked };
}

describe('withRetries', () => {
  it('tries a rate limit, an overload and a dropped connection again, then answers', async () => {
    const { attempt, calls } = flaky([new LlmError(429, 'x'), new LlmError(529, 'x'), new LlmError(null, 'x')]);
    assert.equal(await withRetries(attempt, waits().wait), 'ok');
    assert.equal(calls(), 4);
  });

  it('gives up at once on a request the provider refused', async () => {
    const { attempt, calls } = flaky([new LlmError(400, 'LLM endpoint returned 400')]);
    await assert.rejects(withRetries(attempt, waits().wait), /returned 400/);
    assert.equal(calls(), 1);
  });

  it('stops after the last attempt with the last error', async () => {
    const errors = Array.from({ length: LLM_ATTEMPTS }, () => new LlmError(503, 'LLM endpoint returned 503'));
    const { attempt, calls } = flaky(errors);
    await assert.rejects(withRetries(attempt, waits().wait), /returned 503/);
    assert.equal(calls(), LLM_ATTEMPTS);
  });

  it('waits as long as the provider asks, but never past the cap', async () => {
    const { wait, asked } = waits();
    await withRetries(
      flaky([new LlmError(429, 'x', 2000), new LlmError(429, 'x', 10 * LLM_RETRY_MAX_MS)]).attempt,
      wait,
    );
    assert.deepEqual(asked, [2000, LLM_RETRY_MAX_MS]);
  });
});

describe('the retry rules', () => {
  it('doubles the wait with each retry when the provider gives none', () => {
    const none = new LlmError(500, 'x');
    assert.ok(backoff(2, none, () => 0) === 2 * backoff(1, none, () => 0));
  });

  it('reads a retry-after header in seconds, and ignores one that is not a number', () => {
    assert.equal(retryAfter('3'), 3000);
    assert.equal(retryAfter('soon'), null);
    assert.equal(retryAfter(null), null);
  });

  it('treats a timeout as a failure with no answer, which is worth retrying', () => {
    const err = unanswered(Object.assign(new Error('t'), { name: 'TimeoutError' }));
    assert.deepEqual([err.status, err.message, retryable(err)], [null, 'LLM endpoint did not answer in time', true]);
  });
});
