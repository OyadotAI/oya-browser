/**
 * Unit tests for the browsers' socket entry point: a new socket is handed to a
 * connection, which gives it a deadline to authenticate. Live view, which used
 * to be wired here, is pinned in live-view.test.ts.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { handleConnection, sendCommand, whenConnected } from '../../../../src/modules/browsers/socket.ts';
import { AUTH_DEADLINE_MS, CloseCode } from '../../../../src/modules/browsers/connection/constants.ts';
import { FakeSocket, connectBrowser, disconnectBrowser } from '../../support/fakes.ts';

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

  it('whenConnected answers at once for a browser that is connected', async () => {
    connectBrowser('b-when');
    assert.equal(await whenConnected('b-when', 'key-a', 1000), true);
    disconnectBrowser('b-when');
  });

  it('whenConnected waits for a browser that dropped its socket to come back', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const waiting = whenConnected('b-back', 'key-a', 1000);
    connectBrowser('b-other');
    connectBrowser('b-back');
    assert.equal(await waiting, true);
    disconnectBrowser('b-other');
    disconnectBrowser('b-back');
  });

  it('whenConnected gives up after the wait, so a browser that is gone still fails its command', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const waiting = whenConnected('b-gone', 'key-a', 1000);
    mock.timers.tick(1000);
    assert.equal(await waiting, false);
  });

  it('whenConnected refuses a browser another key registered under the id, waiting or not', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const waiting = whenConnected('b-taken', 'key-a', 1000);
    connectBrowser('b-taken', 'key-b');
    assert.equal(await waiting, false);
    assert.equal(await whenConnected('b-taken', 'key-a', 1000), false);
    disconnectBrowser('b-taken');
  });

  it('whenConnected stops waiting when the run is stopped', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const stop = new AbortController();
    const waiting = whenConnected('b-stopped', 'key-a', 1000, stop.signal);
    stop.abort();
    assert.equal(await waiting, false);
    assert.equal(await whenConnected('b-stopped', 'key-a', 1000, stop.signal), false);
  });
});
