/**
 * Unit tests for the durable record of which browser each chat connector
 * drives: remembered, replaced by a later start, and whether its session may
 * still come back.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { recallSeat, rememberSeat, sessionLive } from '../../../src/mcp/seat-store.ts';
import { stubControl } from '../support/browsers.ts';

describe('seat store', () => {
  afterEach(() => mock.restoreAll());

  it('recalls nothing for a connector that has never started a browser', async () => {
    assert.equal(await recallSeat('seat-never'), undefined);
  });

  it('recalls the browser a connector drives, and the next one after it is replaced', async () => {
    await rememberSeat('seat-a', 'b-1');
    assert.equal(await recallSeat('seat-a'), 'b-1');
    await rememberSeat('seat-a', 'b-2');
    assert.equal(await recallSeat('seat-a'), 'b-2');
  });

  it('counts a session as live until it has stopped or failed, and a missing one as gone', async () => {
    const state = { value: 'ready' as string | null };
    stubControl({ findSession: async () => (state.value ? { state: state.value } : null) });
    assert.equal(await sessionLive('k', 'b-1'), true);
    for (const ended of ['stopped', 'failed', null]) {
      state.value = ended;
      assert.equal(await sessionLive('k', 'b-1'), false, `${ended} is not live`);
    }
  });
});
