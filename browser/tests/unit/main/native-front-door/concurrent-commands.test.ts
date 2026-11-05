/** Both CDP transports must let admitted page commands progress independently of held responses. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { WebSocket } from 'ws';
import { NativeRelayConnection, NativeRelayQueue } from '../../../../src/main/native-front-door/relay.ts';
import { startNativeFrontDoor } from '../../../../src/main/native-front-door/index.ts';
import { NATIVE_DOOR } from '../../../../src/main/native-front-door/constants.ts';
import { flush } from '../../support/fakes.cjs';

/** Tests settle underlying work explicitly; disconnect must not pretend that it cancelled it. */
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

/** Observe correlated replies without making the test await the deliberately held command. */
function client(send: (raw: string) => void, close: () => Promise<void>) {
  let id = 0;
  const replies: any[] = [],
    pending = new Map<number, (reply: any) => void>();
  const receive = (reply: any) => {
    replies.push(reply);
    pending.get(reply.id)?.(reply);
    pending.delete(reply.id);
  };
  const request = (method: string, params = {}, sessionId?: string) =>
    new Promise<any>((resolve) => {
      const requestId = ++id;
      pending.set(requestId, resolve);
      send(JSON.stringify({ id: requestId, method, params, sessionId }));
    });
  return { request, receive, replies, close };
}

/** One backend exposes held reads, promise resolution, and observable native dispatch order. */
async function fixture(t: any, kind: string, strict = false, delayedAdmission = false) {
  const held = deferred(),
    admission = deferred(),
    calls: string[] = [],
    clients: any[] = [];
  let allowed = true,
    begins = 0,
    finishes = 0,
    dispatched = 0;
  const backend: any = {
    targets: () => [{ targetId: 'a', type: 'page', title: '', url: 'https://example.test' }],
    open: async () => 'a',
    close: async () => {},
    execute: async (_target: string, action: string, params: any) => {
      calls.push(action);
      if (action === 'analyze' || params.expression === 'wait') await held.promise;
      return {};
    },
    passesThrough: (method: string) => !strict && /^(Network|Runtime|Page|Fetch)\./.test(method),
    passthrough: async (_target: string, method: string, params: any) => {
      dispatched++;
      calls.push(method);
      if (params.requestId === 'held' || params.expression === 'wait') await held.promise;
      if (params.expression === 'resolve' || method === 'Fetch.fulfillRequest') held.resolve();
      return { value: 'page-result' };
    },
  };
  const beginCommand = async () => {
    if (!allowed) throw Error('Human control');
    begins++;
    if (delayedAdmission) await admission.promise;
    return () => {
      finishes++;
    };
  };
  let connect: (browser?: boolean) => Promise<ReturnType<typeof client>>,
    stop = async () => {};
  if (kind === 'relay') {
    const queue = new NativeRelayQueue();
    connect = async () => {
      const c = client(
        (raw) => connection.send(raw),
        async () => connection.close(),
      );
      const connection = new NativeRelayConnection(
        { backend, allowed: () => allowed, emit: c.receive, closed: () => {} },
        queue,
      );
      clients.push(c);
      return c;
    };
  } else {
    const token = 'x'.repeat(NATIVE_DOOR.tokenLength);
    const server = startNativeFrontDoor({ port: 0, token, backend, beginCommand, clientChanged: () => {} });
    await once(server, 'listening');
    const port = (server.address() as any).port;
    connect = async (browser = false) => {
      const socket = new WebSocket(`ws://127.0.0.1:${port}/devtools/${browser ? 'browser' : 'page/a'}`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      await once(socket, 'open');
      const c = client(
        (raw) => socket.send(raw),
        async () => {
          if (socket.readyState === WebSocket.CLOSED) return;
          socket.close();
          await once(socket, 'close');
        },
      );
      socket.on('message', (raw) => c.receive(JSON.parse(String(raw))));
      clients.push(c);
      return c;
    };
    stop = () => new Promise<void>((resolve) => server.close(() => resolve()));
  }
  t.after(async () => {
    held.resolve();
    admission.resolve();
    await Promise.all(clients.map((c) => c.close()));
    await stop();
  });
  const c = await connect();
  // A real page attachment exercises the same scheduling predicate as browser-session clients.
  if (kind === 'relay') {
    const page = (await c.request('Target.attachToTarget', { targetId: 'a', flatten: true })).result.sessionId;
    const request = c.request;
    c.request = (method: string, params = {}, sessionId?: string) => request(method, params, sessionId ?? page);
  }
  return {
    c,
    connect,
    held,
    admission,
    calls,
    revoke: () => {
      allowed = false;
    },
    counts: () => ({ begins, finishes, dispatched }),
  };
}

for (const kind of ['relay', 'socket']) {
  test(`${kind}: a held body read cannot block page commands or native management`, { timeout: 5000 }, async (t) => {
    const f = await fixture(t, kind);
    const held = f.c.request('Network.getResponseBody', { requestId: 'held' });
    await flush();
    assert.equal((await f.c.request('Page.enable')).error, undefined);
    assert.equal((await f.c.request('Runtime.evaluate', { expression: 'ready' })).error, undefined);
    // Oya operations remain native, even while standard page responses are pending.
    assert.equal((await f.c.request('Oya.getCapabilities')).error, undefined);
    assert.equal(
      f.c.replies.some((reply) => reply.result?.value === 'page-result'),
      true,
    );
    const other = await f.connect(true);
    assert.equal((await other.request('Browser.getVersion')).result.product, 'Oya/native');
    const page = (await other.request('Target.attachToTarget', { targetId: 'a', flatten: true })).result.sessionId;
    assert.equal((await other.request('Page.enable', {}, page)).error, undefined);
    f.held.resolve();
    await held;
  });

  test(`${kind}: a later evaluation can resolve an awaiting page evaluation`, { timeout: 5000 }, async (t) => {
    const f = await fixture(t, kind);
    const waiting = f.c.request('Runtime.evaluate', { expression: 'wait', awaitPromise: true });
    await flush();
    assert.equal((await f.c.request('Runtime.evaluate', { expression: 'resolve' })).error, undefined);
    assert.equal((await waiting).error, undefined);
  });

  test(
    `${kind}: saturation rejects excess work while fulfillment remains able to unblock it`,
    { timeout: 5000 },
    async (t) => {
      const f = await fixture(t, kind);
      const pending = Array.from({ length: NATIVE_DOOR.maxConcurrent }, () =>
        f.c.request('Network.getResponseBody', { requestId: 'held' }),
      );
      await flush();
      const refused = await f.c.request('Page.enable');
      assert.match(refused.error.message, /concurrency limit/);
      assert.equal(f.counts().dispatched, NATIVE_DOOR.maxConcurrent);
      assert.equal(
        (await f.c.request('Fetch.fulfillRequest', { requestId: 'resolve', responseCode: 200 })).error,
        undefined,
      );
      await Promise.all(pending);
      assert.equal((await f.c.request('Page.enable')).error, undefined);
    },
  );

  test(`${kind}: native commands retain completion order while page commands proceed`, { timeout: 5000 }, async (t) => {
    const f = await fixture(t, kind);
    const native = f.c.request('Oya.analyze');
    while (!f.calls.includes('analyze')) await flush();
    const queued = f.c.request('Oya.click', { selector: 'button' });
    await f.c.request('Page.enable');
    assert.deepEqual(f.calls, ['analyze', 'Page.enable']);
    f.held.resolve();
    await Promise.all([native, queued]);
    assert.deepEqual(f.calls, ['analyze', 'Page.enable', 'click']);
  });

  test(
    `${kind}: disconnect suppresses late replies and settlement releases held work once`,
    { timeout: 5000 },
    async (t) => {
      const f = await fixture(t, kind);
      void f.c.request('Network.getResponseBody', { requestId: 'held' });
      while (!f.counts().dispatched) await flush();
      const replies = f.c.replies.length;
      await f.c.close();
      f.held.resolve();
      await flush();
      assert.equal(f.c.replies.length, replies);
      if (kind === 'socket') assert.deepEqual(f.counts(), { begins: 1, finishes: 1, dispatched: 1 });
    },
  );

  test(`${kind}: strict-native mode retains serial execution`, { timeout: 5000 }, async (t) => {
    const f = await fixture(t, kind, true);
    const first = f.c.request('Runtime.evaluate', { expression: 'wait' });
    await flush();
    const second = f.c.request('Runtime.evaluate', { expression: 'ready' });
    await flush();
    assert.deepEqual(f.calls, ['runtime:evaluate']);
    f.held.resolve();
    await Promise.all([first, second]);
    assert.deepEqual(f.calls, ['runtime:evaluate', 'runtime:evaluate']);
  });

  test(`${kind}: disconnect retains bounded slots until the underlying reads settle`, { timeout: 5000 }, async (t) => {
    const f = await fixture(t, kind);
    for (let index = 0; index < NATIVE_DOOR.maxConcurrent; index++)
      void f.c.request('Network.getResponseBody', { requestId: 'held' });
    while (f.counts().dispatched < NATIVE_DOOR.maxConcurrent) await flush();
    await f.c.close();
    const other = await f.connect(true);
    const page = (await other.request('Target.attachToTarget', { targetId: 'a', flatten: true })).result.sessionId;
    assert.match((await other.request('Page.enable', {}, page)).error.message, /concurrency limit/);
    f.held.resolve();
    await flush();
    assert.equal((await other.request('Page.enable', {}, page)).error, undefined);
  });
}

test('loopback disconnect during admission finishes once without dispatching', { timeout: 5000 }, async (t) => {
  const f = await fixture(t, 'socket', false, true);
  void f.c.request('Page.enable');
  while (!f.counts().begins) await flush();
  await f.c.close();
  f.admission.resolve();
  await flush();
  assert.deepEqual(f.counts(), { begins: 1, finishes: 1, dispatched: 0 });
});

test(
  'relay takeover denies new passed work and suppresses an in-flight sensitive result',
  { timeout: 5000 },
  async (t) => {
    const f = await fixture(t, 'relay');
    const held = f.c.request('Network.getResponseBody', { requestId: 'held' });
    await flush();
    f.revoke();
    assert.match((await f.c.request('Page.enable')).error.message, /unavailable/);
    f.held.resolve();
    assert.match((await held).error.message, /revoked/);
    assert.equal(JSON.stringify(f.c.replies).includes('page-result'), false);
  },
);
