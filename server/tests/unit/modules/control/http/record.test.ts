/**
 * Unit tests for recordFlow, POST /control/sessions/:id/record: the known
 * modes run against the flow recorder, anything else is refused.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../../support/data-dir.ts';

ownDataDir('oya-control-http-record-');
const { recordFlow } = await import('../../../../../src/modules/control/http/record.ts');

/** A record request for browser b-rec. */
const request = (body) => ({ authToken: 'oya_alice', params: { id: 'b-rec' }, body });

describe('recordFlow', () => {
  it('refuses an unknown mode', async () => {
    await assert.rejects(recordFlow('key-a', request({ mode: 'pause' })), { status: 400, code: 'invalid_mode' });
    await assert.rejects(recordFlow('key-a', request({ mode: 'constructor' })), { status: 400 });
    await assert.rejects(recordFlow('key-a', request(undefined)), { status: 400 });
  });

  it('reports no recording for a browser that is not recording', async () => {
    assert.deepEqual(await recordFlow('key-a', request({ mode: 'status' })), {
      recording: false,
      steps: [],
      secrets: [],
    });
  });

  it('discards nothing without complaint when there is no recording', async () => {
    assert.deepEqual(await recordFlow('key-a', request({ mode: 'discard' })), { ok: true });
  });
});
