/**
 * Unit tests for the CDP relay: CDP carried over a browser's control socket,
 * opened on the browser's confirmation and closed by either side, a timeout
 * or the socket going away.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { WebSocket } from 'ws';
import { closeRelays, onBrowserMessage, openRelay } from '../../../../src/modules/browsers/cdp-relay.ts';
import { FakeSocket } from '../../support/fakes.ts';

const B = 'b-relay';

/** Opens a relay on a fake socket; returns the pending relay and the relay id it asked for. */
function open(timeoutMs?: number) {
  const ws = new FakeSocket();
  const pending: Promise<any> = openRelay({ ws }, B, timeoutMs);
  const [ask] = ws.ofType('cdp_open');
  return { ws, pending, sid: ask?.sid };
}

/** A relay the browser has confirmed. */
async function opened() {
  const { ws, pending, sid } = open();
  onBrowserMessage(B, { type: 'cdp_opened', sid });
  return { ws, relay: await pending, sid };
}

describe('CDP relay', () => {
  afterEach(() => mock.timers.reset());

  it('asks the browser to open, and resolves open once it confirms', async () => {
    const { relay } = await opened();
    assert.equal(relay.readyState, WebSocket.OPEN);
    assert.equal(relay.browserId, B);
  });

  it('sends each CDP frame with the relay id', async () => {
    const { ws, relay, sid } = await opened();
    relay.send(Buffer.from('{"id":1}'));
    assert.deepEqual(ws.ofType('cdp')[0], { type: 'cdp', data: '{"id":1}', sid });
  });

  it('emits the browser’s CDP frames as messages', async () => {
    const { relay, sid } = await opened();
    const got = [];
    relay.on('message', (data, isBinary) => got.push([String(data), isBinary]));
    onBrowserMessage(B, { type: 'cdp', sid, data: '{"result":{}}' });
    assert.deepEqual(got, [['{"result":{}}', false]]);
  });

  it('ignores a message naming another browser or an unknown relay', async () => {
    const { relay, sid } = await opened();
    const got = mock.fn();
    relay.on('message', got);
    onBrowserMessage('someone-else', { type: 'cdp', sid, data: 'x' });
    onBrowserMessage(B, { type: 'cdp', sid: 'nope', data: 'x' });
    onBrowserMessage(B, { type: 'unknown', sid });
    assert.equal(got.mock.callCount(), 0);
  });

  it('tells the browser when it closes, and closes only once', async () => {
    const { ws, relay } = await opened();
    const closes = mock.fn();
    relay.on('close', closes);
    relay.terminate();
    relay.close();
    assert.equal(ws.ofType('cdp_close').length, 1);
    assert.equal(closes.mock.callCount(), 1);
    assert.equal(relay.readyState, WebSocket.CLOSED);
  });

  it('rejects with the browser’s reason when it refuses', async () => {
    const { pending, sid } = open();
    onBrowserMessage(B, { type: 'cdp_closed', sid, error: 'CDP port closed' });
    await assert.rejects(pending, /CDP port closed/);
  });

  it('gives up when the browser does not confirm in time', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const { pending } = open(500);
    mock.timers.tick(500);
    await assert.rejects(pending, /Browser did not open CDP/);
  });

  it('fails at once when the control socket cannot be written', async () => {
    const ws = new FakeSocket();
    ws.failWith = new Error('closed');
    await assert.rejects(openRelay({ ws }, B), /Browser disconnected/);
  });

  it('closes the relay when a frame cannot be sent', async () => {
    const { ws, relay } = await opened();
    ws.failWith = new Error('closed');
    relay.send('x');
    assert.equal(relay.readyState, WebSocket.CLOSED);
    assert.equal(relay.error, 'Browser disconnected');
  });

  it('closes every relay of a browser whose socket went away', async () => {
    const { relay } = await opened();
    closeRelays(B);
    assert.equal(relay.readyState, WebSocket.CLOSED);
    assert.equal(relay.error, 'Browser disconnected');
  });
});
