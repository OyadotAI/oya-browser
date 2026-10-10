/** Remote native dispatch must retain queue bounds, revocation and cleanup around asynchronous renderer work. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { NativeRelayConnection, NativeRelayQueue } from '../../../../src/main/native-front-door/relay.ts';
import { NATIVE_DOOR } from '../../../../src/main/native-front-door/constants.ts';
import { flush } from '../../support/fakes.cjs';

/** Hold a native operation independently of the transport's scheduling queue. */
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

/** Native operation seams expose only observable dispatch and cleanup effects. */
function fixture() {
  const held = deferred();
  const replies: any[] = [],
    operations: string[] = [];
  let allowed = true,
    disposed = 0,
    closed = 0;
  const backend: any = {
    targets: () => [{ targetId: 'page', type: 'page', title: '', url: 'https://example.test' }],
    execute: async (_id: string, operation: string) => {
      operations.push(operation);
      if (operation === 'runtime:evaluate') await held.promise;
      return { secret: 'native-result' };
    },
    resolveRequest: () => {
      operations.push('continue');
      held.resolve();
    },
    dispose: () => {
      disposed++;
    },
  };
  const connection = new NativeRelayConnection(
    {
      backend,
      allowed: () => allowed,
      emit: (m) => replies.push(m),
      closed: () => {
        closed++;
      },
    },
    new NativeRelayQueue(),
  );
  const send = (id: number, method: string, params = {}, sessionId?: string) =>
    connection.send(JSON.stringify({ id, method, params, sessionId }));
  const attach = async () => {
    send(1, 'Target.attachToTarget', { targetId: 'page', flatten: true });
    await flush();
    return replies.at(-1).result.sessionId as string;
  };
  return {
    connection,
    send,
    attach,
    held,
    replies,
    operations,
    revoke: () => {
      allowed = false;
    },
    counts: () => ({ disposed, closed }),
  };
}

it('rejects queue saturation and recovers capacity after native work completes', async () => {
  const queue = new NativeRelayQueue(),
    held = deferred();
  let executed = 0;
  const pending = Array.from({ length: NATIVE_DOOR.maxQueued }, () =>
    queue.run(async () => {
      executed++;
      await held.promise;
    }),
  );
  await assert.rejects(
    queue.run(async () => {
      executed++;
    }),
    /queue is full/,
  );
  held.resolve();
  await Promise.all(pending);
  await flush();
  await queue.run(async () => {
    executed++;
  });
  assert.equal(executed, NATIVE_DOOR.maxQueued + 1);
});

it('in-flight takeover returns an error without exposing the completed native result', async () => {
  const f = fixture(),
    sessionId = await f.attach();
  f.send(2, 'Runtime.evaluate', { expression: 'secret' }, sessionId);
  await flush();
  assert.deepEqual(f.operations, ['runtime:evaluate']);
  f.revoke();
  f.held.resolve();
  await flush();
  assert.match(f.replies.at(-1).error.message, /revoked/);
  assert.equal(JSON.stringify(f.replies).includes('native-result'), false);
  f.connection.close();
});

it('disconnect disposes once and suppresses late native success and queued execution', async () => {
  const f = fixture(),
    sessionId = await f.attach();
  f.send(2, 'Runtime.evaluate', { expression: 'secret' }, sessionId);
  await flush();
  f.send(3, 'Input.insertText', { text: 'queued' }, sessionId);
  f.connection.close();
  f.connection.close();
  f.held.resolve();
  await flush();
  assert.deepEqual(f.operations, ['runtime:evaluate']);
  assert.equal(f.replies.length, 1);
  assert.deepEqual(f.counts(), { disposed: 1, closed: 1 });
});

it('continuation bypass cannot execute after human takeover', async () => {
  const f = fixture(),
    sessionId = await f.attach();
  f.send(2, 'Runtime.evaluate', { expression: 'secret' }, sessionId);
  await flush();
  f.revoke();
  f.send(3, 'Fetch.continueRequest', { requestId: 'held' }, sessionId);
  await flush();
  assert.equal(f.replies.at(-1).id, 3);
  assert.match(f.replies.at(-1).error.message, /unavailable/);
  assert.deepEqual(f.operations, ['runtime:evaluate']);
  f.connection.close();
  f.held.resolve();
});

it('authorized native continuations release held work without waiting behind that work', async () => {
  const f = fixture(),
    sessionId = await f.attach();
  f.send(2, 'Runtime.evaluate', { expression: 'secret' }, sessionId);
  await flush();
  f.send(3, 'Fetch.continueRequest', { requestId: 'held' }, sessionId);
  await flush();
  assert.deepEqual(f.operations, ['runtime:evaluate', 'continue']);
  assert.ok(f.replies.some((reply) => reply.id === 2 && reply.result));
  assert.ok(f.replies.some((reply) => reply.id === 3 && reply.result));
  f.connection.close();
});
