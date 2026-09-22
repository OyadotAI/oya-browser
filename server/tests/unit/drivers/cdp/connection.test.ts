/**
 * Unit tests for the CDP JSON-RPC transport over a stand-in socket: requests
 * matched to replies by id, events to listeners, timeouts, and failing every
 * request once the socket goes.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { WebSocket } from 'ws';
import { CDPConnection } from '../../../../src/drivers/cdp/connection.ts';

/** A connection over a socket that records what it writes. */
function open() {
  const conn = new CDPConnection('ws://unused');
  const written: any[] = [];
  const socket = { readyState: 1, send: (data: string) => written.push(JSON.parse(data)), close: mock.fn() };
  conn.ws = socket as any;
  return { conn, written, socket };
}

/** Delivers a raw message to the connection, as the socket would. */
const deliver = (conn: CDPConnection, message: unknown) => conn.onMessage(Buffer.from(JSON.stringify(message)));

describe('CDPConnection.over', () => {
  it('speaks CDP on a socket already open, and fails pending sends when it closes', async () => {
    const socket: any = new EventEmitter();
    socket.readyState = WebSocket.OPEN;
    socket.sent = [];
    socket.send = (data: string) => socket.sent.push(JSON.parse(data));
    socket.close = () => {};
    const conn = CDPConnection.over(socket);
    const answered = conn.send('Browser.getVersion');
    socket.emit('message', Buffer.from(JSON.stringify({ id: socket.sent[0].id, result: { product: 'X' } })));
    assert.deepEqual(await answered, { product: 'X' });
    const pending = conn.send('Page.navigate', { url: 'https://a.test' });
    socket.readyState = WebSocket.CLOSED;
    socket.emit('close');
    await assert.rejects(pending, { message: 'CDP connection closed' });
    await assert.rejects(conn.send('Browser.getVersion'), /closed/);
  });
});

describe('CDPConnection', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
  afterEach(() => mock.timers.reset());

  it('sends numbered requests and resolves each with its own reply', async () => {
    const { conn, written } = open();
    const first = conn.send('Page.navigate', { url: 'u' }, 'sess');
    const second = conn.send('Browser.getVersion');
    assert.deepEqual(written, [
      { id: 1, method: 'Page.navigate', params: { url: 'u' }, sessionId: 'sess' },
      { id: 2, method: 'Browser.getVersion', params: {} },
    ]);
    deliver(conn, { id: 2, result: { product: 'Chrome' } });
    deliver(conn, { id: 1, result: { frameId: 'f' } });
    assert.deepEqual(await first, { frameId: 'f' });
    assert.deepEqual(await second, { product: 'Chrome' });
  });

  it('rejects a request the browser answered with an error', async () => {
    const { conn } = open();
    const pending = conn.send('Page.navigate');
    deliver(conn, { id: 1, error: { message: 'Invalid URL' } });
    await assert.rejects(pending, { message: 'Invalid URL' });
  });

  it('ignores replies nobody waits for and messages that are not JSON', () => {
    const { conn } = open();
    deliver(conn, { id: 99, result: {} });
    conn.onMessage(Buffer.from('not json'));
    assert.equal(conn.pending.size, 0);
  });

  it('times out a request that gets no reply', async () => {
    const { conn } = open();
    const pending = conn.send('Page.reload', {}, undefined, 500);
    mock.timers.tick(500);
    await assert.rejects(pending, { message: 'CDP Page.reload timed out' });
    assert.equal(conn.pending.size, 0);
  });

  it('fails at once when the socket is closed or refuses the write', async () => {
    const closed = open();
    closed.conn.close();
    await assert.rejects(closed.conn.send('Page.reload'), { message: 'CDP connection closed' });
    assert.equal(closed.socket.close.mock.callCount(), 1);

    const broken = open();
    broken.socket.send = () => {
      throw new Error('write EPIPE');
    };
    await assert.rejects(broken.conn.send('Page.reload'), { message: 'write EPIPE' });
    assert.equal(broken.conn.pending.size, 0);
  });

  it('fails every in-flight request together', async () => {
    const { conn } = open();
    const a = conn.send('A');
    const b = conn.send('B');
    conn.failAll(new Error('gone'));
    await assert.rejects(a, /gone/);
    await assert.rejects(b, /gone/);
  });

  it('hands events to every listener, even when one throws, until unsubscribed', () => {
    const { conn } = open();
    const seen: any[] = [];
    conn.on('Page.loadEventFired', () => {
      throw new Error('bad listener');
    });
    const off = conn.on('Page.loadEventFired', (params, sid) => seen.push([params, sid]));
    deliver(conn, { method: 'Page.loadEventFired', params: { t: 1 }, sessionId: 's' });
    off();
    deliver(conn, { method: 'Page.loadEventFired', params: { t: 2 } });
    assert.deepEqual(seen, [[{ t: 1 }, 's']]);
  });

  it('waits for one event, or resolves null once the wait runs out', async () => {
    const { conn } = open();
    const fired = conn.once('Page.loadEventFired', 1000);
    deliver(conn, { method: 'Page.loadEventFired', params: { t: 1 } });
    assert.deepEqual(await fired, { t: 1 });
    const missed = conn.once('Page.loadEventFired', 1000);
    mock.timers.tick(1000);
    assert.equal(await missed, null);
    assert.equal(conn.listeners.get('Page.loadEventFired')!.size, 0);
  });
});
