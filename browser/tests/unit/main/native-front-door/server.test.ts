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
  const connect = async () => {
    const socket = new WebSocket(`ws://127.0.0.1:${port}/devtools/page/a`, {
      headers: { authorization: `Bearer ${token}` },
    });
    await once(socket, 'open');
    t.after(() => socket.terminate());
    return socket;
  };
  return { port, token, calls, connect };
}
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
