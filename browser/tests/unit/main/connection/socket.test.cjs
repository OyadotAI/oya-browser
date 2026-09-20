/**
 * Unit tests for ControlSocket: auth on open, messages handled in order,
 * sends that never throw, backoff and fatal close codes, and the heartbeat
 * with its proxy-byte reports.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { ControlSocket, reconnectDelay, randomId, authMessage } = require('../../../../main/connection/socket.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');

/** A ws client stand-in: records sends and closes. */
class FakeWs extends EventEmitter {
  /** ws's OPEN state. */
  static OPEN = 1;
  /** Every socket made, in order. */
  static made = [];
  /** Opens (in name) to `url`. */
  constructor(url) {
    super();
    this.url = url;
    this.readyState = FakeWs.OPEN;
    this.sent = [];
    this.closes = [];
    FakeWs.made.push(this);
  }

  /** Records a frame; `failSend` makes it throw. */
  send(text) {
    if (this.failSend) throw new Error('socket gone');
    this.sent.push(JSON.parse(text));
  }

  /** Records a close. */
  close(code, reason) {
    this.closes.push([code, reason]);
  }
}

describe('ControlSocket', () => {
  let ctx, bytes;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout', 'setInterval'] });
    FakeWs.made = [];
    bytes = [];
    ctx = mainCtx();
    ctx.config.values = { apiKey: 'k', serverUrl: 'ws://s.test/ws', browserName: 'Desk' };
    ctx.relay = { closeCdpRelays() {} };
    ctx.cookies = { flushCookieChanges() {} };
    ctx.stream = { stopStream() {} };
    ctx.socket = new ControlSocket(ctx, { WebSocketImpl: FakeWs, takeBytes: () => bytes.shift() || 0 });
  });
  afterEach(() => mock.timers.reset());

  it('does not connect without a key', () => {
    ctx.config.values.apiKey = '';
    ctx.socket.connect();
    assert.equal(FakeWs.made.length, 0);
  });

  it('authenticates on open with a stable browser id', () => {
    ctx.socket.connect();
    FakeWs.made[0].emit('open');
    const auth = FakeWs.made[0].sent[0];
    assert.equal(auth.type, 'auth');
    assert.match(auth.browser_id, /^oya-[0-9a-f]{16}$/);
    ctx.socket.connect();
    assert.equal(ctx.socket.browserId, auth.browser_id);
  });

  it('describes this browser in the auth message', () => {
    const msg = authMessage({ apiKey: 'k', browserName: 'n', persona: 'p', provider: 'oya-cloud' }, 'b', 9222);
    assert.deepEqual(msg, {
      type: 'auth',
      api_key: 'k',
      browser_id: 'b',
      browser_name: 'n',
      persona: 'p',
      provider: 'oya-cloud',
      enrollment_token: process.env.OYA_ENROLLMENT_TOKEN,
      cdp: true,
    });
  });

  it('never throws from send, and says whether it went', () => {
    assert.equal(ctx.socket.send({ type: 'x' }), false);
    ctx.socket.connect();
    assert.equal(ctx.socket.send({ type: 'x' }), true);
    FakeWs.made[0].failSend = true;
    assert.equal(ctx.socket.send({ type: 'x' }), false);
  });

  it('handles messages one at a time, in order, skipping garbage', async () => {
    const seen = [];
    ctx.shell.browsingMode = true;
    ctx.stream = { startStream: (fps) => seen.push(fps), stopStream() {} };
    ctx.socket.connect();
    const ws = FakeWs.made[0];
    ws.emit('message', Buffer.from('not json'));
    ws.emit('message', Buffer.from(JSON.stringify({ type: 'stream_start', fps: 5 })));
    ws.emit('message', Buffer.from(JSON.stringify({ type: 'stream_start' })));
    await flush();
    assert.deepEqual(seen, [5, 2]);
  });

  it('closes a session whose setup failed', async () => {
    ctx.control.result = () => {
      throw new Error('bad');
    };
    ctx.socket.connect();
    FakeWs.made[0].emit('message', Buffer.from(JSON.stringify({ type: 'desktop_control_result' })));
    await flush();
    assert.deepEqual(FakeWs.made[0].closes, [[4003, 'Session setup failed']]);
  });

  it('reconnects with backoff after an ordinary close', () => {
    mock.method(Math, 'random', () => 0);
    ctx.socket.connect();
    FakeWs.made[0].emit('close', 1006);
    assert.equal(ctx.socket.reconnectAttempts, 1);
    mock.timers.tick(reconnectDelay(1) - 1);
    assert.equal(FakeWs.made.length, 1);
    mock.timers.tick(1);
    assert.equal(FakeWs.made.length, 2);
  });

  it('does not reconnect after a replaced session or a rejected key', () => {
    mock.method(console, 'log', () => {});
    for (const code of [4000, 4001, 4003]) {
      ctx.socket.connect();
      FakeWs.made.at(-1).emit('close', code);
      assert.equal(ctx.socket.reconnectTimer, null);
    }
  });

  it('caps the backoff and adds jitter', () => {
    mock.method(Math, 'random', () => 1);
    assert.equal(reconnectDelay(1), 750 + 500);
    assert.equal(reconnectDelay(50), 10000 + 500);
    assert.match(randomId(), /^oya-[0-9a-f]{16}$/);
  });

  it('disconnects without hearing its own close, and stops the session', () => {
    ctx.socket.connect();
    const ws = FakeWs.made[0];
    ctx.socket.disconnect();
    ws.emit('close', 1000);
    assert.equal(ctx.socket.ws, null);
    assert.equal(ctx.socket.reconnectTimer, null);
    assert.deepEqual(ctx.shell.sentOn('ws-status').at(-1), {
      connected: false,
      browserId: ctx.socket.browserId,
      profileName: 'Default',
    });
  });

  it('pings, and closes a server that stopped answering', () => {
    ctx.socket.connect();
    ctx.socket.startPingLoop();
    mock.timers.tick(20000 * 4);
    assert.equal(FakeWs.made[0].sent.filter((m) => m.type === 'ping').length, 4);
    mock.timers.tick(20000);
    assert.equal(FakeWs.made[0].closes.length, 1);
  });

  it('keeps proxy bytes a failed send could not report, for the next beat', () => {
    ctx.socket.connect();
    ctx.socket.startPingLoop();
    const ws = FakeWs.made[0];
    bytes.push(100);
    ws.readyState = 0;
    mock.timers.tick(20000);
    assert.equal(ctx.socket.proxyBytesUnsent, 100);
    ws.readyState = FakeWs.OPEN;
    mock.timers.tick(20000);
    assert.deepEqual(
      ws.sent.filter((m) => m.type === 'proxy_bytes'),
      [{ type: 'proxy_bytes', bytes: 100 }],
    );
    assert.equal(ctx.socket.proxyBytesUnsent, 0);
  });
});
