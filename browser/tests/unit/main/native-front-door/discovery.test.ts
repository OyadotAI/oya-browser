/** Native discovery emits scoped lifecycle changes, cancels subscriptions, and never uses a debugging target source. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { NativeProtocol } from '../../../../src/main/native-front-door/protocol.ts';
/** Native registry seam, with explicit tab-change notifications and independent sockets. */
function fixture() {
  const listeners = new Set<() => void>(),
    events: any[] = [],
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
  const protocol = new NativeProtocol(backend, undefined, (event) => events.push(event));
  const run = (method: string, params: any = {}, sessionId?: string) =>
    protocol.dispatch({ id: 1, method, params, sessionId });
  return {
    backend,
    protocol,
    run,
    events,
    activated,
    listeners,
    change(next: typeof targets) {
      targets = next;
      for (const notify of listeners) notify();
    },
  };
}
test('discovery emits initial, changed and removed targets without duplicate events', async () => {
  const f = fixture();
  await f.run('Target.setDiscoverTargets', { discover: true });
  assert.equal(f.events[0].method, 'Target.targetCreated');
  assert.equal(f.events[0].params.targetInfo.attached, false);
  await f.run('Target.setDiscoverTargets', { discover: true });
  assert.equal(f.events.length, 1);
  f.change([{ targetId: 'a', type: 'page', url: 'https://example.test/next', title: 'Next' }]);
  assert.equal(f.events[1].method, 'Target.targetInfoChanged');
  f.change([]);
  assert.deepEqual(f.events[2], { method: 'Target.targetDestroyed', params: { targetId: 'a' } });
  await f.run('Target.setDiscoverTargets', { discover: false });
  assert.equal(f.listeners.size, 0);
});
test('destroyed targets revoke attached sessions and emit scoped detach before destruction', async () => {
  const f = fixture();
  await f.run('Target.setDiscoverTargets', { discover: true });
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  f.change([]);
  assert.deepEqual(f.events.at(-2), { method: 'Target.detachedFromTarget', params: { targetId: 'a', sessionId } });
  await assert.rejects(f.run('DOM.getDocument', {}, sessionId), /outside/);
  f.protocol.dispose();
  assert.equal(f.listeners.size, 0);
});
test('two sockets have independent discovery and disconnect removes only its own listener', async () => {
  const f = fixture(),
    otherEvents: any[] = [];
  const other = new NativeProtocol(f.backend, undefined, (e) => otherEvents.push(e));
  await f.run('Target.setDiscoverTargets', { discover: true });
  await other.dispatch({ id: 1, method: 'Target.setDiscoverTargets', params: { discover: true } });
  f.protocol.dispose();
  assert.equal(f.listeners.size, 1);
  f.change([]);
  assert.equal(otherEvents.at(-1).method, 'Target.targetDestroyed');
  assert.equal(f.events.length, 1);
  other.dispose();
  assert.equal(f.listeners.size, 0);
});
test('activation pins the requested native target and page sessions cannot discover other tabs', async () => {
  const f = fixture();
  await f.run('Target.activateTarget', { targetId: 'a' });
  const { sessionId } = (await f.run('Target.attachToTarget', { targetId: 'a', flatten: true })) as any;
  await f.run('Page.bringToFront', {}, sessionId);
  assert.deepEqual(f.activated, ['a', 'a']);
  await assert.rejects(f.run('Target.setDiscoverTargets', { discover: true }, sessionId), /browser endpoint/);
  await assert.rejects(f.run('Target.activateTarget', { targetId: 'foreign' }), /outside/);
});
test('discovery refuses missing booleans and unsupported filters rather than silently changing scope', async () => {
  const f = fixture();
  for (const params of [{}, { discover: 'true' }, { discover: true, filter: [{ type: 'worker' }] }])
    await assert.rejects(f.run('Target.setDiscoverTargets', params));
  assert.equal(f.listeners.size, 0);
});
