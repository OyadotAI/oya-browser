/** Browser-root sessions preserve standard CDP attachment without sharing another scope's state. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeProtocol } from '../../../../src/main/native-front-door/protocol.ts';
import { NATIVE_DOOR } from '../../../../src/main/native-front-door/constants.ts';

/** An authorized native tab with connection-local protocol events and a real passthrough seam. */
function fixture() {
  const events: any[] = [],
    calls: any[] = [],
    listeners = new Set<() => void>();
  let tabs = [{ targetId: 'page', type: 'page' as const, title: '', url: 'https://example.test' }];
  let disposed = 0,
    connections = 0,
    stops = 0;
  const backend: any = {
    targets: () => tabs,
    watchTargets: (changed: () => void) => (listeners.add(changed), () => void listeners.delete(changed)),
    open: async () => 'page',
    close: async () => {},
    execute: async (...args: any[]) => (calls.push(args), {}),
    passesThrough: (method: string) => /^(Network|Page|Runtime)\./.test(method),
    passthrough: async (...args: any[]) => (calls.push(args), { passed: true }),
    passthroughEvents: (_target: string, emit: any) => {
      calls.push(['watch', emit]);
      return () => void stops++;
    },
    dispose: () => void disposed++,
    connection: () => (connections++, backend),
  };
  const protocol = new NativeProtocol(backend, undefined, (event) => events.push(event));
  const run = (method: string, params: any = {}, sessionId?: string): Promise<any> =>
    protocol.dispatch({ id: 1, method, params, sessionId });
  const browser = async (parent?: string) => (await run('Target.attachToBrowserTarget', {}, parent)).sessionId;
  const page = async (parent?: string) =>
    (await run('Target.attachToTarget', { targetId: 'page', flatten: true }, parent)).sessionId;
  return {
    protocol,
    backend,
    run,
    browser,
    page,
    events,
    calls,
    listeners,
    counts: () => ({ disposed, connections, stops }),
    closeTarget: () => {
      tabs = [];
      for (const changed of [...listeners]) changed();
    },
  };
}

test('a browser session supports the exact page CDP setup used by SiteTwin', async () => {
  const f = fixture(),
    root = await f.browser(),
    page = await f.page(root);
  for (const [method, params] of [
    ['Network.enable', {}],
    ['Network.setBypassServiceWorker', { bypass: true }],
    ['Network.setCacheDisabled', { cacheDisabled: true }],
  ] as const)
    assert.deepEqual(await f.run(method, params, page), { passed: true });
  assert.equal((await f.run('Target.getTargetInfo', {}, root)).targetInfo.type, 'browser');
  assert.equal(f.counts().connections, 1);
  f.protocol.dispose();
  assert.equal(f.counts().disposed, 1);
});

test('native attachments and child aliases share capacity and child detach returns it', async () => {
  const f = fixture(),
    page = await f.page();
  await f.run('Network.enable', {}, page);
  const fire = f.calls.find((call) => call[0] === 'watch')[1];
  for (let index = 1; index < NATIVE_DOOR.maxSessions; index++)
    fire('Target.attachedToTarget', { sessionId: `child-${index}` }, '');
  await assert.rejects(f.browser(), /limit/);
  fire('Target.detachedFromTarget', { sessionId: 'child-1' }, '');
  await f.browser();
  f.protocol.dispose();
});

test('child observer overflow reports through the fatal transport path and leaves no late aliases', async () => {
  const f = fixture(),
    failures: string[] = [];
  const protocol = new NativeProtocol(
    f.backend,
    undefined,
    () => {},
    (reason) => {
      failures.push(reason);
      protocol.dispose();
    },
  );
  const page = (
    (await protocol.dispatch({
      id: 1,
      method: 'Target.attachToTarget',
      params: { targetId: 'page', flatten: true },
    })) as any
  ).sessionId;
  await protocol.dispatch({ id: 2, method: 'Network.enable', params: {}, sessionId: page });
  const fire = f.calls.find((call) => call[0] === 'watch')[1];
  for (let index = 0; index < NATIVE_DOOR.maxSessions; index++)
    fire('Target.attachedToTarget', { sessionId: `child-${index}` }, '');
  assert.equal(failures.length, 1);
  fire('Target.attachedToTarget', { sessionId: 'late' }, '');
  assert.equal(failures.length, 1);
  await assert.rejects(protocol.dispatch({ id: 3, method: 'Browser.getVersion', params: {} }), /closed/);
  f.protocol.dispose();
});

test('browser-session page events keep the page session and detach events keep its browser parent', async () => {
  const f = fixture(),
    root = await f.browser(),
    page = await f.page(root);
  await f.run('Network.enable', {}, page);
  f.calls.find((call) => call[0] === 'watch')[1]('Network.loadingFinished', { requestId: 'request' }, '');
  assert.equal(f.events.at(-1).sessionId, page);
  await f.run('Target.detachFromTarget', { sessionId: page }, root);
  assert.deepEqual(f.events.at(-1), {
    method: 'Target.detachedFromTarget',
    params: { sessionId: page, targetId: 'page' },
    sessionId: root,
  });
  await assert.rejects(f.run('Network.enable', {}, page), /outside|Unknown/);
  f.protocol.dispose();
});

test('detaching a browser revokes its descendants and preserves sibling automatic page sessions', async () => {
  const f = fixture();
  await f.run('Target.setAutoAttach', { autoAttach: true, flatten: true, waitForDebuggerOnStart: false });
  const sibling = f.events.find((event) => event.method === 'Target.attachedToTarget').params.sessionId;
  const root = await f.browser(),
    nested = await f.browser(root),
    page = await f.page(nested);
  await f.run('Network.enable', {}, page);
  await f.run('Target.detachFromTarget', { sessionId: root });
  for (const session of [root, nested, page])
    await assert.rejects(f.run('Target.getTargets', {}, session), /outside|Unknown/);
  assert.deepEqual(await f.run('Network.enable', {}, sibling), { passed: true });
  assert.equal(f.counts().stops, 1);
  f.protocol.dispose();
});

test('browser scopes own independent discovery and automatic attachment observers', async () => {
  const f = fixture(),
    first = await f.browser(),
    second = await f.browser();
  for (const root of [first, second]) {
    await f.run('Target.setDiscoverTargets', { discover: true }, root);
    await f.run('Target.setAutoAttach', { autoAttach: true, flatten: true, waitForDebuggerOnStart: false }, root);
  }
  assert.deepEqual(
    f.events.filter((event) => event.method === 'Target.targetCreated').map((event) => event.sessionId),
    [first, second],
  );
  const attached = f.events.filter((event) => event.method === 'Target.attachedToTarget');
  assert.equal(new Set(attached.map((event) => event.params.sessionId)).size, 2);
  await f.run('Target.setAutoAttach', { autoAttach: false, flatten: true, waitForDebuggerOnStart: false }, first);
  assert.deepEqual(
    await f.run('Network.enable', {}, attached.find((event) => event.sessionId === second).params.sessionId),
    { passed: true },
  );
  f.protocol.dispose();
  assert.equal(f.listeners.size, 0);
});

test('target closure revokes every page attachment and notifies each browser parent', async () => {
  const f = fixture(),
    first = await f.browser(),
    second = await f.browser();
  const pages = [await f.page(first), await f.page(second)];
  f.closeTarget();
  const detached = f.events.filter((event) => event.method === 'Target.detachedFromTarget');
  assert.deepEqual(new Set(detached.map((event) => event.sessionId)), new Set([first, second]));
  for (const page of pages) await assert.rejects(f.run('Network.enable', {}, page), /outside|Unknown/);
  assert.equal((await f.run('Target.getTargets', {}, first)).targetInfos.length, 0);
  f.protocol.dispose();
});

test('foreign, stale, page and pinned sessions cannot acquire browser authority', async () => {
  const f = fixture(),
    other = fixture(),
    root = await f.browser(),
    page = await f.page(root);
  await assert.rejects(other.run('Target.getTargets', {}, root), /outside|Unknown/);
  await assert.rejects(f.run('Target.attachToBrowserTarget', {}, page), /browser endpoint/);
  await assert.rejects(f.run('Network.enable', {}, root), /Unsupported|outside|target|page/i);
  await assert.rejects(f.run('Target.detachFromTarget', { sessionId: page }), /scope|parent|Unknown/);
  const pinned = new NativeProtocol(f.backend, 'page');
  await assert.rejects(
    pinned.dispatch({ id: 1, method: 'Target.attachToBrowserTarget', params: {} }),
    /browser endpoint/,
  );
  pinned.dispose();
  f.protocol.dispose();
  other.protocol.dispose();
});

test('browser and page sessions share the existing capacity and release it after detach', async () => {
  const f = fixture();
  const roots = [];
  for (let n = 0; n < NATIVE_DOOR.maxSessions; n++) roots.push(await f.browser());
  await assert.rejects(f.browser(), /limit/);
  await assert.rejects(f.page(roots[0]), /limit/);
  await f.run('Target.detachFromTarget', { sessionId: roots[0] });
  assert.equal(typeof (await f.page(roots[1])), 'string');
  f.protocol.dispose();
});

test('browser attachments reject parameters and cleanup is idempotent', async () => {
  const f = fixture();
  await assert.rejects(f.run('Target.attachToBrowserTarget', { targetId: 'page' }), /Unsupported parameter/);
  const root = await f.browser(),
    page = await f.page(root);
  await f.run('Network.enable', {}, page);
  f.protocol.dispose();
  f.protocol.dispose();
  assert.deepEqual(f.counts(), { connections: 1, disposed: 1, stops: 1 });
  assert.equal(f.listeners.size, 0);
  await assert.rejects(f.run('Target.getTargets', {}, root), /outside|Unknown|closed/);
});

test('failed page observer setup releases its attachment capacity', async () => {
  const f = fixture(),
    root = await f.browser();
  f.backend.watchTargets = () => {
    throw Error('observer unavailable');
  };
  await assert.rejects(f.page(root), /observer unavailable/);
  for (let n = 1; n < NATIVE_DOOR.maxSessions; n++) await f.browser();
  await assert.rejects(f.browser(), /limit/);
  f.protocol.dispose();
});

test('browser management events retain the requesting browser session', async () => {
  const f = fixture(),
    root = await f.browser();
  let notify: any;
  f.backend.manage = async (_action: string, _params: object, emit: any) => {
    notify = emit;
    emit('Browser.downloadWillBegin', { guid: 'download' });
    return {};
  };
  await f.run('Target.getBrowserContexts', {}, root);
  assert.deepEqual(f.events.at(-1), {
    method: 'Browser.downloadWillBegin',
    params: { guid: 'download' },
    sessionId: root,
  });
  await f.run('Target.detachFromTarget', { sessionId: root });
  const eventsAfterDetach = f.events.length;
  notify('Browser.downloadWillBegin', { guid: 'late-download' });
  assert.equal(f.events.length, eventsAfterDetach);
  f.protocol.dispose();
});
