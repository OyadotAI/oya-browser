/**
 * Unit tests for the IPC contract: no two calls or events share a channel.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { CALL_CHANNELS, EVENT_CHANNELS } from '../../../src/shared/ipc.ts';

describe('IPC contract', () => {
  it('gives every call and every event a channel of its own', () => {
    for (const table of [CALL_CHANNELS, EVENT_CHANNELS]) {
      const channels = Object.values(table);
      assert.equal(new Set(channels).size, channels.length);
    }
  });
});
