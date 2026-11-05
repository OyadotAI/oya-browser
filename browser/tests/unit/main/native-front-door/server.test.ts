/** The real loopback WebSocket boundary requires authentication and holds ownership until native work settles. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { WebSocket } from 'ws';
import { startNativeFrontDoor } from '../../../../src/main/native-front-door/index.ts';
/** Local-only fixture; no public network and no page engine is needed for transport authorization. */
async function fixture(
  t: any,
  execute = async () => ({}),
  resolveRequest?: (target: string, id: string, cancel: boolean) => void,
  passthrough?: (...args: any[]) => Promise<unknown>,
) {
  const token = 'x'.repeat(32),
    calls: string[] = [];
  const server = startNativeFrontDoor({
    port: 0,
    token,
    backend: {
      targets: () => [{ targetId: 'a', type: 'page', title: '', url: 'https://example.test' }],
      execute,
      resolveRequest,
      passesThrough: (method: string) => !!passthrough && method === 'Fetch.fulfillRequest',
      passthrough,
      open: async () => 'a',
      close: async () => {},
    },
    beginCommand: () => {
      calls.push('begin');
      return () => {
        calls.push('finish');
      };
    },
    clientChanged: (delta) => {
      calls.push(`client:${delta}`);
    },
  });
  await once(server, 'listening');
  t.after(() => new Promise<void>((resolve) => server.close(() => resolve())));
  const port = (server.address() as any).port;
  const connect = async (browser = false) => {
    const socket = new WebSocket(`ws://127.0.0.1:${port}/devtools/${browser ? 'browser' : 'page/a'}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    await once(socket, 'open');
    t.after(() => socket.terminate());
    return socket;
  };
  return { port, token, calls, connect };
}
/** Correlate replies while allowing independent session events to arrive before them. */
async function request(socket: WebSocket, message: any): Promise<any> {
  return await new Promise((resolve) => {
    const receive = (raw: any) => {
      const reply = JSON.parse(String(raw));
      if (reply.id !== message.id) return;
      socket.off('message', receive);
      resolve(reply);
    };
    socket.on('message', receive);
    socket.send(JSON.stringify(message));
  });
}
test('browser and page session success, errors and detach events retain their wire scope', async (t) => {
  const f = await fixture(t),
    socket = await f.connect(true),
    events: any[] = [];
  socket.on('message', (raw) => {
    const message = JSON.parse(String(raw));
    if (message.method) events.push(message);
  });
  const root = (await request(socket, { id: 1, method: 'Target.attachToBrowserTarget' })).result.sessionId;
  const attachment = await request(socket, {
    id: 2,
    method: 'Target.attachToTarget',
    params: { targetId: 'a', flatten: true },
    sessionId: root,
  });
  assert.equal(attachment.sessionId, root);
  const page = attachment.result.sessionId;
  const success = await request(socket, { id: 3, method: 'DOM.getDocument', sessionId: page });
  assert.deepEqual(success, { id: 3, sessionId: page, result: {} });
  const failure = await request(socket, { id: 4, method: 'Browser.getVersion', sessionId: page });
  assert.equal(failure.sessionId, page);
  assert.match(failure.error.message, /browser endpoint/);
  await request(socket, { id: 5, method: 'Target.detachFromTarget', params: { sessionId: page }, sessionId: root });
  assert.deepEqual(events.at(-1), {
    method: 'Target.detachedFromTarget',
    params: { sessionId: page, targetId: 'a' },
    sessionId: root,
  });
  socket.close();
  await once(socket, 'close');
});
test('discovery refuses missing credentials and browser origins before revealing targets', async (t) => {
  const f = await fixture(t);
  const url = `http://127.0.0.1:${f.port}/json/list`;
  assert.equal((await fetch(url)).status, 403);
  assert.equal(
    (await fetch(url, { headers: { authorization: `Bearer ${f.token}`, Origin: 'https://evil.test' } })).status,
    403,
  );
  const response = await fetch(url, { headers: { authorization: `Bearer ${f.token}` } });
  assert.equal(response.status, 200);
  assert.equal(((await response.json()) as any[])[0].id, 'a');
  assert.deepEqual(f.calls, []);
});
test('native results use the external WebSocket and always release the command gate', async (t) => {
  const f = await fixture(t),
    socket = await f.connect();
  const reply = once(socket, 'message');
  socket.send(JSON.stringify({ id: 1, method: 'DOM.getDocument' }));
  assert.deepEqual(JSON.parse(String((await reply)[0])), { id: 1, result: {} });
  assert.deepEqual(f.calls, ['client:1', 'begin', 'finish']);
  socket.close();
  await once(socket, 'close');
});
test('disconnect drops queued work but never releases an in-flight native operation early', async (t) => {
  let complete!: () => void, started!: () => void;
  const begun = new Promise<void>((r) => {
    started = r;
  });
  const pending = new Promise<void>((r) => {
    complete = r;
  });
  let executed = 0;
  const f = await fixture(t, async () => {
    executed++;
    started();
    await pending;
    return {};
  });
  const socket = await f.connect();
  socket.send(JSON.stringify({ id: 1, method: 'DOM.getDocument' }));
  socket.send(JSON.stringify({ id: 2, method: 'DOM.getDocument' }));
  await begun;
  socket.close();
  await once(socket, 'close');
  assert.equal(f.calls.includes('finish'), false);
  complete();
  await new Promise((r) => setImmediate(r));
  assert.equal(executed, 1);
  assert.equal(f.calls.filter((c) => c === 'finish').length, 1);
});

test('native request continuation unblocks held work without releasing or reacquiring its gate', async (t) => {
  let resume!: () => void, start!: () => void;
  const started = new Promise<void>((r) => {
    start = r;
  });
  const held = new Promise<void>((r) => {
    resume = r;
  });
  const f = await fixture(
    t,
    async () => {
      start();
      await held;
      return {};
    },
    (target, id, cancel) => {
      assert.equal(target, 'a');
      assert.equal(id, 'held-request');
      assert.equal(cancel, false);
      assert.deepEqual(f.calls, ['client:1', 'begin']);
      resume();
    },
  );
  const socket = await f.connect();
  const replies = new Map();
  const complete = new Promise<void>((resolve) =>
    socket.on('message', (raw) => {
      const reply = JSON.parse(String(raw));
      replies.set(reply.id, reply);
      if (replies.size === 2) resolve();
    }),
  );
  socket.send(JSON.stringify({ id: 1, method: 'DOM.getDocument' }));
  await started;
  socket.send(JSON.stringify({ id: 2, method: 'Fetch.continueRequest', params: { requestId: 'held-request' } }));
  await complete;
  assert.equal(replies.get(1).error, undefined);
  assert.equal(replies.get(2).error, undefined);
  assert.deepEqual(f.calls, ['client:1', 'begin', 'finish']);
  socket.close();
  await once(socket, 'close');
});

test('response fulfillment runs while its navigation holds the command gate', async (t) => {
  let resume!: () => void, started!: () => void;
  const begun = new Promise<void>((resolve) => {
    started = resolve;
  });
  const held = new Promise<void>((resolve) => {
    resume = resolve;
  });
  const f = await fixture(
    t,
    async () => {
      started();
      await held;
      return {};
    },
    undefined,
    async (target, method) => {
      assert.equal(target, 'a');
      assert.equal(method, 'Fetch.fulfillRequest');
      assert.deepEqual(f.calls, ['client:1', 'begin']);
      resume();
      return {};
    },
  );
  const socket = await f.connect();
  const pending = request(socket, { id: 1, method: 'DOM.getDocument' });
  await begun;
  const fulfilled = await request(socket, {
    id: 2,
    method: 'Fetch.fulfillRequest',
    params: { requestId: 'held-request', responseCode: 200, body: 'b2s=' },
  });
  assert.equal(fulfilled.error, undefined);
  assert.equal((await pending).error, undefined);
  assert.deepEqual(f.calls, ['client:1', 'begin', 'finish']);
  socket.close();
  await once(socket, 'close');
});
