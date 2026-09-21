/**
 * Unit tests for launching a batch of Oya Cloud sandboxes: every create is
 * settled on its own, and the outcome is booked in usage, metrics and the
 * audit log.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { auditProvision, bookProvision, createMany } from '../../../../../src/modules/browsers/lifecycle/provision.ts';
import * as usage from '../../../../../src/platform/usage.ts';
import { recent } from '../../../../../src/platform/audit.ts';
import { reset as resetMetrics, snapshot } from '../../../../../src/platform/metrics.ts';
import { fakeRequest } from '../../../support/browsers.ts';

describe('provision', () => {
  afterEach(() => mock.restoreAll());

  it('settles every create, reporting each failure’s reason', async () => {
    // Oya Cloud is not configured in tests, so each create fails on its own.
    const { created, failed } = await createMany('key-a', 'n', 3);
    assert.equal(created.length, 0);
    assert.equal(failed.length, 3);
    assert.ok(failed.every((f) => typeof f === 'string' && f.length > 0));
  });

  it('books created sandboxes in usage and both outcomes in metrics', () => {
    usage.reset();
    resetMetrics();
    mock.method(console, 'error', () => {});
    bookProvision('key-p', 3, ['a', 'b'], ['boom']);
    assert.equal(usage.current('key-p').sandboxes_created, 2);
    const series = snapshot().oya_sandboxes_total;
    assert.equal(series.find((s) => s.labels.outcome === 'ok').value, 2);
    assert.equal(series.find((s) => s.labels.outcome === 'error').value, 1);
  });

  it('audits a provision as failed only when nothing was created', () => {
    auditProvision(fakeRequest(), 'key-a', 2, [], ['x', 'y']);
    auditProvision(fakeRequest(), 'key-a', 2, ['a'], ['y']);
    const [partial, none] = recent({ action: 'browser.provision' });
    assert.equal(none.outcome, 'error');
    assert.equal(partial.outcome, 'ok');
    assert.deepEqual(partial.meta, { requested: 2, created: 1, failed: 1 });
  });
});
