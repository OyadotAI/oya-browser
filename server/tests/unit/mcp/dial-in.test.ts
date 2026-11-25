/**
 * Unit tests for waiting on a started cloud browser: connected at once, or
 * given up on after the start wait.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { connects } from '../../../src/mcp/dial-in.ts';
import { START_POLL_MS, START_WAIT_MS } from '../../../src/mcp/constants.ts';
import { connectBrowser, disconnectBrowser } from '../support/fakes.ts';

describe('connects', () => {
  afterEach(() => {
    mock.timers.reset();
    disconnectBrowser('d-1');
  });

  it('is true for a browser that has dialled in', async () => {
    connectBrowser('d-1');
    assert.equal(await connects('d-1'), true);
  });

  it('is false once the start wait passes without it', async () => {
    mock.timers.enable({ apis: ['setTimeout', 'Date'] });
    const waiting = connects('d-1');
    for (let t = 0; t <= START_WAIT_MS; t += START_POLL_MS) {
      await new Promise((r) => setImmediate(r));
      mock.timers.tick(START_POLL_MS);
    }
    assert.equal(await waiting, false);
  });
});
