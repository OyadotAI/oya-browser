/**
 * Unit tests for the browsers' socket entry point: a new socket is handed to a
 * connection, which gives it a deadline to authenticate. Live view, which used
 * to be wired here, is pinned in live-view.test.ts.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { handleConnection, sendCommand } from '../../../../src/modules/browsers/socket.ts';
import { AUTH_DEADLINE_MS, CloseCode } from '../../../../src/modules/browsers/connection/constants.ts';
import { FakeSocket } from '../../support/fakes.ts';

/** A FakeSocket that accepts the listeners a connection subscribes. */
class ListeningSocket extends FakeSocket {
  /** Where the connection's listeners live. */
  events = new EventEmitter();

  /** Subscribes, as ws.on does. */
  on(event: string, fn: (...args: any[]) => void) {
    this.events.on(event, fn);
  }
}

describe('socket entry point', () => {
  afterEach(() => mock.timers.reset());

  it('takes over a new socket and closes it when it does not authenticate in time', () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const ws = new ListeningSocket();
    handleConnection(ws);
    assert.equal(ws.closed, null);
    mock.timers.tick(AUTH_DEADLINE_MS);
    assert.deepEqual(ws.closed, { code: CloseCode.AUTH_TIMEOUT, reason: 'Auth timeout' });
  });

  it('is where the rest of the server gets sendCommand from', () => {
    assert.equal(typeof sendCommand, 'function');
  });
});
