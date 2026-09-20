/**
 * Unit tests for queued starts: each claimed job is replayed against the start
 * handler and its answer recorded, up to a batch per pass, with an outcome that
 * cannot be recorded marked unknown.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-provision-');
const { startProvisioning } = await import('../../../../../src/modules/control/worker/provision.ts');
const { flags, workerHealth } = await import('../../../../../src/modules/control/worker/state.ts');

/** A job whose start the handler refuses without calling any provider: its persona does not exist. */
const job = (id) => ({
  key: 'key-a',
  session: { id, fence: 4 },
  request: { provider: 'cdp', persona: 'no-such-persona' },
});

/** A service whose queue hands out `jobs` in turn. */
function fakeService(jobs, complete = async () => {}) {
  return {
    claimQueued: mock.fn(async () => jobs.shift() ?? null),
    complete: mock.fn(complete),
    update: mock.fn(async () => {}),
  };
}

/** Starts a pass and waits for it. */
async function pass(service) {
  startProvisioning(service);
  await flags.provisioning;
}

let lastError;
beforeEach(() => {
  lastError = workerHealth.lastError;
});
afterEach(() => {
  workerHealth.lastError = lastError;
  mock.restoreAll();
});

describe('startProvisioning', () => {
  it('stops when nothing is queued', async () => {
    const service = fakeService([]);
    await pass(service);
    assert.equal(service.claimQueued.mock.callCount(), 1);
    assert.equal(flags.provisioning, null);
  });

  it('replays each claimed start and records what the handler answered', async () => {
    const service = fakeService([job('q1')]);
    await pass(service);
    const [key, id, status, body] = service.complete.mock.calls[0].arguments;
    assert.deepEqual([key, id, status], ['key-a', 'q1', 404]);
    assert.ok(body.error);
  });

  it('starts at most eight per pass', async () => {
    const service = fakeService(Array.from({ length: 10 }, (_, i) => job(`q${i}`)));
    await pass(service);
    assert.equal(service.complete.mock.callCount(), 8);
  });

  it('marks the outcome unknown, under the job’s fence, when it cannot be recorded', async () => {
    const service = fakeService([job('q1')], async () => {
      throw new Error('storage down');
    });
    await pass(service);
    assert.deepEqual(service.update.mock.calls[0].arguments, [
      'key-a',
      'q1',
      { state: 'unknown_outcome' },
      { fence: 4 },
    ]);
  });

  it('keeps a failed claim for the health check', async () => {
    await pass({
      claimQueued: async () => {
        throw new Error('queue unreadable');
      },
    });
    assert.equal(workerHealth.lastError, 'queue unreadable');
  });

  it('never runs two passes at once', async () => {
    const service = fakeService([]);
    startProvisioning(service);
    startProvisioning(service);
    await flags.provisioning;
    assert.equal(service.claimQueued.mock.callCount(), 1);
  });
});
