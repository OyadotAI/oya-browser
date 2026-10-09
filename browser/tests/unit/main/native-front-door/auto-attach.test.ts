/** Native discovery emits scoped lifecycle changes, cancels subscriptions, and never uses a debugging target source. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeProtocol } from '../../../../src/main/native-front-door/protocol.ts';
/** Native registry seam, with explicit tab-change notifications and independent sockets. */
function fixture() {
  const listeners = new Set<() => void>(),
    events: any[] = [],
    failures: string[] = [],
    activated: string[] = [];
  let targets = [{ targetId: 'a', type: 'page' as const, url: 'https://example.test', title: 'A' }];
  const backend = {
    targets: () => targets,
    watchTargets: (listener: () => void) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    activate: (id: string) => {
      activated.push(id);
    },
    open: async () => 'unused',
    close: async () => {},
    execute: async () => ({}),
  };
  const protocol = new NativeProtocol(
    backend,
    undefined,
    (event) => events.push(event),
    (reason) => failures.push(reason),
  );
  const run = (method: string, params: any = {}, sessionId?: string) =>
    protocol.dispatch({ id: 1, method, params, sessionId });
  return {
    backend,
    protocol,
    run,
    events,
    failures,
    activated,
    listeners,
    change(next: typeof targets) {
      targets = next;
      for (const notify of listeners) notify();
    },
  };
}
/** The only supported auto-attachment contract is explicit, unpaused page sessions. */
const enabled = { autoAttach: true, flatten: true, waitForDebuggerOnStart: false, filter: [{ type: 'page' }] };
test('automatic attachment covers existing and future pages once and revokes closed sessions', async () => {
  const f = fixture();
  await f.run('Target.setAutoAttach', enabled);
  const first = f.events[0].params.sessionId;
  await f.run('Target.setAutoAttach', enabled);
  assert.equal(f.events.length, 1);
  f.change([{ targetId: 'b', type: 'page', title: 'B', url: 'https://example.test/b' }]);
  assert.equal(f.events[1].method, 'Target.detachedFromTarget');
  assert.equal(f.events[2].method, 'Target.attachedToTarget');
  await assert.rejects(f.run('DOM.getDocument', {}, first), /outside/);
  f.protocol.dispose();
  assert.equal(f.listeners.size, 0);
});
test('disabling automatic attachment preserves separately attached manual sessions', async () => {
  const f = fixture();
  const manual = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  await f.run('Target.setAutoAttach', enabled);
  await f.run('Target.setAutoAttach', { ...enabled, autoAttach: false });
  await f.run('DOM.getDocument', {}, manual.sessionId);
  assert.equal(f.events.filter((e) => e.method === 'Target.detachedFromTarget').length, 1);
  assert.equal(f.listeners.size, 0);
});
test('explicit detach forgets automatic ownership and discovery never duplicates detach events', async () => {
  const f = fixture();
  await f.run('Target.setDiscoverTargets', { discover: true });
  await f.run('Target.setAutoAttach', enabled);
  const id = f.events.find((e) => e.method === 'Target.attachedToTarget').params.sessionId;
  await f.run('Target.detachFromTarget', { sessionId: id });
  f.change(f.backend.targets());
  assert.equal(f.events.filter((e) => e.method === 'Target.attachedToTarget').length, 2);
  f.change([]);
  assert.equal(f.events.filter((e) => e.method === 'Target.detachedFromTarget').length, 1);
  f.protocol.dispose();
});
test('unsupported automatic attachment contracts fail without subscribing', async () => {
  const f = fixture();
  for (const params of [
    {},
    { ...enabled, filter: undefined },
    { ...enabled, flatten: false },
    { ...enabled, waitForDebuggerOnStart: true },
    { ...enabled, filter: [{ type: 'worker' }] },
    { ...enabled, filter: [{ type: 'page', exclude: true }] },
  ])
    await assert.rejects(f.run('Target.setAutoAttach', params));
  assert.equal(f.listeners.size, 0);
});
test('capacity preflight rolls back initial setup and future overflow explicitly fails the transport', async () => {
  const f = fixture();
  const many = Array.from({ length: 129 }, (_, i) => ({
    targetId: String(i),
    type: 'page' as const,
    url: 'about:blank',
    title: '',
  }));
  f.change(many);
  await assert.rejects(f.run('Target.setAutoAttach', enabled), /limit/);
  assert.equal(f.events.length, 0);
  assert.equal(f.listeners.size, 0);
  f.change(many.slice(0, 1));
  await f.run('Target.setAutoAttach', enabled);
  f.change(many);
  assert.equal(f.failures.length, 1);
  assert.equal(f.listeners.size, 0);
  assert.equal(f.events.filter((e) => e.method === 'Target.attachedToTarget').length, 1);
});
test('automatic sessions cannot be used by a different socket', async () => {
  const f = fixture();
  await f.run('Target.setAutoAttach', enabled);
  const other = new NativeProtocol(f.backend);
  await assert.rejects(
    other.dispatch({ id: 1, method: 'DOM.getDocument', params: {}, sessionId: f.events[0].params.sessionId }),
    /outside/,
  );
  other.dispose();
  assert.equal(f.listeners.size, 1);
  f.protocol.dispose();
});
