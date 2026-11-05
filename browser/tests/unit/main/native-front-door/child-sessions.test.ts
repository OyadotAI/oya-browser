/** Opaque child routes keep page attachments from replacing each other's nested iframe or worker sessions. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { PassthroughSessions } from '../../../../src/main/native-front-door/passthrough.ts';
import { NATIVE_DOOR } from '../../../../src/main/native-front-door/constants.ts';

/** Fan out one exact tab's debugger events as the real backend does. */
function fixture() {
  const watches = new Set<(method: string, params: object, child: string) => void>();
  const callbacks: ((method: string, params: object, child: string) => void)[] = [];
  const emitted: any[] = [],
    sent: unknown[][] = [];
  const backend = {
    targets: () => [],
    open: async () => '',
    close: async () => {},
    execute: async () => ({}),
    passesThrough: () => true,
    passthrough: async (...args: unknown[]) => (sent.push(args), {}),
    passthroughEvents: (_target: string, emit: any) => (
      callbacks.push(emit),
      watches.add(emit),
      () => void watches.delete(emit)
    ),
  };
  const sessions = new PassthroughSessions(backend, (event) => emitted.push(event));
  const send = (sessionId: string, method = 'Page.enable', params = {}, target = 'tab') =>
    sessions.send({ id: 1, sessionId, method, params }, () => target);
  const fire = (method: string, params: object, child = '') => {
    for (const emit of watches) emit(method, params, child);
  };
  const attach = (raw: string, parent = '') =>
    fire('Target.attachedToTarget', { sessionId: raw, targetInfo: { targetId: raw + '-target' } }, parent);
  return { sessions, emitted, sent, send, fire, attach, callbacks };
}

test('two page watches receive independent iframe aliases and nested worker envelopes', async () => {
  const f = fixture();
  await f.send('one');
  await f.send('two');
  f.attach('iframe');
  const [one, two] = f.emitted.map((event) => event.params.sessionId);
  assert.notEqual(one, two);
  assert.notEqual(one, 'iframe');
  f.attach('worker', 'iframe');
  const workers = f.emitted.slice(2);
  assert.deepEqual(
    workers.map((event) => event.sessionId),
    [one, two],
  );
  assert.notEqual(workers[0].params.sessionId, workers[1].params.sessionId);
  await f.send(workers[0].params.sessionId, 'Runtime.enable');
  assert.deepEqual(f.sent.at(-1), ['tab', 'Runtime.enable', {}, 'worker']);
});

test('duplicate or conflicting attachment announcements cannot replace an existing alias', async () => {
  const f = fixture();
  await f.send('one');
  f.attach('iframe');
  f.attach('iframe');
  f.attach('worker', 'iframe');
  f.attach('iframe', 'worker');
  assert.equal(f.emitted.length, 2);
  assert.equal(f.sessions.size, 2);
});

test('child detach translates only the session parameter and requires its immediate parent', async () => {
  const f = fixture();
  await f.send('one');
  await f.send('two');
  f.attach('iframe');
  f.attach('worker', 'iframe');
  const iframe = f.emitted[0].params.sessionId,
    worker = f.emitted[2].params.sessionId;
  await assert.rejects(async () => f.send('one', 'Target.detachFromTarget', { sessionId: worker }));
  await assert.rejects(async () => f.send('two', 'Target.detachFromTarget', { sessionId: iframe }));
  await assert.rejects(async () => f.send('one', 'Target.detachFromTarget', { sessionId: 'iframe' }));
  await f.send(iframe, 'Target.detachFromTarget', { sessionId: worker, targetId: 'untouched' });
  assert.deepEqual(f.sent.at(-1), [
    'tab',
    'Target.detachFromTarget',
    { sessionId: 'worker', targetId: 'untouched' },
    'iframe',
  ]);
});

test('releasing one watch preserves its sibling aliases while late callbacks are ignored', async () => {
  const f = fixture();
  await f.send('one');
  await f.send('two');
  f.attach('iframe');
  const sibling = f.emitted[1].params.sessionId;
  f.sessions.release('one');
  f.fire('Runtime.consoleAPICalled', {}, 'iframe');
  assert.equal(f.emitted.at(-1).sessionId, sibling);
  await f.send(sibling, 'Runtime.enable');
  assert.deepEqual(f.sent.at(-1), ['tab', 'Runtime.enable', {}, 'iframe']);
  f.sessions.dispose();
  for (const callback of f.callbacks) callback('Target.attachedToTarget', { sessionId: 'late' }, '');
  assert.equal(f.sessions.size, 0);
});

test('the same raw session on different tabs cannot cross-route commands', async () => {
  const f = fixture();
  await f.send('one', 'Page.enable', {}, 'tab-one');
  await f.send('two', 'Page.enable', {}, 'tab-two');
  f.attach('iframe');
  await f.send(f.emitted[0].params.sessionId, 'Runtime.enable');
  await f.send(f.emitted[1].params.sessionId, 'Runtime.enable');
  assert.deepEqual(f.sent.slice(-2), [
    ['tab-one', 'Runtime.enable', {}, 'iframe'],
    ['tab-two', 'Runtime.enable', {}, 'iframe'],
  ]);
});

test('nested detach translates session fields while retaining target identities', async () => {
  const f = fixture();
  await f.send('one');
  f.attach('iframe');
  f.attach('worker', 'iframe');
  const iframe = f.emitted[0].params.sessionId,
    worker = f.emitted[1].params.sessionId;
  f.fire('Target.detachedFromTarget', { sessionId: 'worker', targetId: 'worker-target' }, 'iframe');
  assert.deepEqual(f.emitted.at(-1), {
    method: 'Target.detachedFromTarget',
    params: { sessionId: worker, targetId: 'worker-target' },
    sessionId: iframe,
  });
  assert.equal(f.sessions.handles({ id: 1, method: 'Runtime.enable', params: {}, sessionId: worker }, false), false);
});

test('synchronous observer revocation still stops the listener returned after its callback', async () => {
  let stops = 0,
    calls = 0;
  const backend = {
    targets: () => [],
    open: async () => '',
    close: async () => {},
    execute: async () => ({}),
    passthrough: async () => (calls++, {}),
    passthroughEvents: (_target: string, emit: any) => {
      for (let index = 0; index <= NATIVE_DOOR.maxSessions; index++)
        emit('Target.attachedToTarget', { sessionId: `child-${index}` }, '');
      return () => void stops++;
    },
  };
  const sessions = new PassthroughSessions(backend, () => {});
  assert.throws(
    () => sessions.send({ id: 1, method: 'Page.enable', params: {}, sessionId: 'page' }, () => 'tab'),
    /closed/,
  );
  assert.equal(stops, 1);
  assert.equal(sessions.size, 0);
  assert.equal(calls, 0);
});

test('a new watch cannot receive an existing foreign raw child through another watch', async () => {
  const f = fixture();
  await f.send('one');
  f.attach('iframe');
  await f.send('two');
  f.fire('Runtime.consoleAPICalled', {}, 'iframe');
  assert.equal(f.emitted.filter((event) => event.method === 'Runtime.consoleAPICalled').length, 1);
});

test('detaching an iframe revokes its worker aliases without revoking sibling page routes', async () => {
  const f = fixture();
  await f.send('one');
  await f.send('two');
  f.attach('iframe');
  f.attach('worker', 'iframe');
  const worker = f.emitted[2].params.sessionId;
  f.fire('Target.detachedFromTarget', { sessionId: 'iframe' });
  assert.equal(f.sessions.handles({ id: 1, method: 'Runtime.enable', params: {}, sessionId: worker }, false), false);
  f.sessions.release('one');
  assert.equal(f.sessions.handles({ id: 1, method: 'Page.enable', params: {}, sessionId: 'two' }, true), true);
});
