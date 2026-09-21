/**
 * Unit tests for the worker's shared state: the flags that keep each kind of
 * background work from overlapping itself, and the health record.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { exclusive, flags, recordError, workerHealth } from '../../../../../src/modules/control/worker/state.ts';

describe('exclusive', () => {
  it('skips work while the same kind is already running', async () => {
    let runs = 0;
    let finish;
    const first = exclusive('validating', () => new Promise((resolve) => (finish = resolve)).then(() => runs++));
    await exclusive('validating', async () => runs++);
    assert.equal(runs, 0);
    finish();
    await first;
    assert.equal(runs, 1);
    assert.equal(flags.validating, false);
  });

  it('releases the flag when the work throws', async () => {
    await assert.rejects(
      exclusive('heartbeating', async () => {
        throw new Error('boom');
      }),
      /boom/,
    );
    assert.equal(flags.heartbeating, false);
  });
});

describe('recordError', () => {
  it('keeps a background failure for the health check', () => {
    const before = workerHealth.lastError;
    recordError(new Error('lease write failed'));
    assert.equal(workerHealth.lastError, 'lease write failed');
    workerHealth.lastError = before;
  });
});
