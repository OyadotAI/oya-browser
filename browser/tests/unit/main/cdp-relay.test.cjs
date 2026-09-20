/**
 * Unit tests for main/cdp-relay.cjs: a relayed CDP socket is opened onto the
 * front door with the relay token and its life is reported over the control
 * socket. The front door and the socket are local fakes.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const http = require('node:http');
const { WebSocketServer } = require('ws');
const { createCdpRelay } = require('../../../main/cdp-relay.cjs');
const { flush } = require('../support/fakes.cjs');

/** A loopback front door: /json/version and a WebSocket that echoes and records its headers. */
async function fakeFrontDoor() {
  const server = http.createServer((req, res) => {
    res.end(JSON.stringify({ webSocketDebuggerUrl: 'ws://ignored:1/devtools/browser/abc' }));
  });
  const wss = new WebSocketServer({ server });
  const seen = [];
  wss.on('connection', (sock, req) => {
    seen.push({ url: req.url, relay: req.headers['x-oya-relay'], sock });
    sock.on('message', (data) => sock.send('echo:' + data));
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const close = () => {
    for (const s of seen) s.sock.terminate();
    wss.close();
    server.close();
  };
  return { port: server.address().port, seen, close };
}

/** Waits until `check()` holds. */
async function until(check) {
  while (!check()) await new Promise((resolve) => setTimeout(resolve, 5));
}

describe('createCdpRelay', () => {
  afterEach(() => mock.restoreAll());

  it('reports that CDP is off when no front door runs', async () => {
    const sent = [];
    const relay = createCdpRelay({ port: 0, token: 't', send: (m) => sent.push(m) });
    await relay.openCdpRelay('s1');
    assert.deepEqual(sent, [
      {
        type: 'cdp_closed',
        sid: 's1',
        error: 'CDP is off in this browser. Start it with OYA_REMOTE_DEBUGGING_PORT set.',
      },
    ]);
  });

  it('reports a front door that cannot be reached', async () => {
    mock.method(globalThis, 'fetch', async () => {
      throw new Error('ECONNREFUSED');
    });
    const sent = [];
    const relay = createCdpRelay({ port: 9, token: 't', send: (m) => sent.push(m) });
    await relay.openCdpRelay('s1');
    assert.deepEqual(sent, [{ type: 'cdp_closed', sid: 's1', error: 'ECONNREFUSED' }]);
    assert.equal(relay.cdpRelays.size, 0);
  });

  it('opens the browser target with the relay token and carries messages both ways', async () => {
    const door = await fakeFrontDoor();
    const sent = [];
    const relay = createCdpRelay({ port: door.port, token: 'secret', send: (m) => sent.push(m) });
    try {
      await relay.openCdpRelay('s1');
      await until(() => sent.some((m) => m.type === 'cdp_opened'));
      assert.deepEqual(
        door.seen.map((s) => [s.url, s.relay]),
        [['/devtools/browser/abc', 'secret']],
      );
      relay.cdpRelays.get('s1').send('hi');
      await until(() => sent.some((m) => m.type === 'cdp'));
      assert.deepEqual(sent.at(-1), { type: 'cdp', sid: 's1', data: 'echo:hi' });
    } finally {
      relay.closeCdpRelays();
      door.close();
    }
  });

  it('tells the server when the local socket closes, once', async () => {
    const door = await fakeFrontDoor();
    const sent = [];
    const relay = createCdpRelay({ port: door.port, token: 't', send: (m) => sent.push(m) });
    try {
      await relay.openCdpRelay('s1');
      await until(() => door.seen.length === 1);
      door.seen[0].sock.close();
      await until(() => sent.some((m) => m.type === 'cdp_closed'));
      await flush();
      assert.equal(sent.filter((m) => m.type === 'cdp_closed').length, 1);
      assert.equal(relay.cdpRelays.size, 0);
    } finally {
      door.close();
    }
  });

  it('closes every relayed socket and forgets them', () => {
    const closed = [];
    const relay = createCdpRelay({ port: 1, token: 't', send: () => true });
    relay.cdpRelays.set('a', { close: () => closed.push('a') });
    relay.cdpRelays.set('b', {
      close: () => {
        throw new Error('already closed');
      },
    });
    relay.closeCdpRelays();
    assert.deepEqual(closed, ['a']);
    assert.equal(relay.cdpRelays.size, 0);
  });
});
