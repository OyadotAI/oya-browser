/**
 * Unit tests for Heartbeat: it pings on an interval and closes a socket that
 * has gone quiet.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Heartbeat } from '../../../../../src/modules/browsers/connection/heartbeat.ts';
import { CloseCode, MISSED_PINGS, PING_INTERVAL_MS } from '../../../../../src/modules/browsers/connection/constants.ts';
import { FakeSocket } from '../../../support/fakes.ts';

describe('Heartbeat', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setInterval', 'Date'] }));
  afterEach(() => mock.timers.reset());

  it('pings every interval while the browser answers', () => {
    const ws = new FakeSocket();
    const beat = new Heartbeat(ws, () => 'b');
    beat.start();
    mock.timers.tick(PING_INTERVAL_MS * 2);
    assert.equal(ws.ofType('ping').length, 2);
    beat.stop();
  });

  it('closes the socket once pongs stop for the allowed misses', () => {
    const ws = new FakeSocket();
    const beat = new Heartbeat(ws, () => 'b');
    mock.method(console, 'log', () => {});
    beat.start();
    mock.timers.tick(PING_INTERVAL_MS * (MISSED_PINGS + 1));
    assert.deepEqual(ws.closed, { code: CloseCode.PONG_TIMEOUT, reason: 'Pong timeout' });
  });

  it('stays open while the browser keeps being heard', () => {
    const ws = new FakeSocket();
    const beat = new Heartbeat(ws, () => 'b');
    beat.start();
    for (let i = 0; i <= MISSED_PINGS * 2; i++) {
      mock.timers.tick(PING_INTERVAL_MS);
      beat.heard();
    }
    assert.equal(ws.closed, null);
    beat.stop();
  });
});
