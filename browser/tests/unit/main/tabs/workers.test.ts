/** All worker types inherit native protection from the exact session before starting. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { WorkerCoverage } from '../../../../src/main/tabs/workers.ts';
test('worker startup verifies the active exact session without endpoint discovery', async () => {
  const session = {};
  let checked;
  const coverage = new WorkerCoverage({
    persona: { session: () => session },
    protection: {
      assertSession(value) {
        checked = value;
      },
    },
  });
  await coverage.cover();
  assert.equal(checked, session);
  coverage.stop();
});
test('missing native protection rejects worker startup instead of logging and continuing', async () => {
  const coverage = new WorkerCoverage({
    persona: { session: () => ({}) },
    protection: {
      assertSession() {
        throw Error('not protected');
      },
    },
  });
  await assert.rejects(coverage.cover(), /not protected/);
});
