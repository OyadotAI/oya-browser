/** Passed-through page sessions keep to their own tab, and to the iframe and worker sessions it attaches. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { PassthroughSessions } from '../../../../src/main/native-front-door/passthrough.ts';

/** A backend that records passthrough calls and lets the test fire tab events. */
function fixture() {
  const sent: unknown[][] = [],
    emitted: any[] = [];
  let fire: (method: string, params: object, child: string) => void = () => {};
  let stops = 0;
  const backend = {
    targets: () => [],
    open: async () => '',
    close: async () => {},
    execute: async () => ({}),
    passesThrough: (method: string) =>
      method.startsWith('Page.') || method.startsWith('Runtime.') || method === 'Target.setAutoAttach',
    passthrough: async (...args: unknown[]) => (sent.push(args), {}),
    passthroughEvents: (_target: string, emit: any) => ((fire = emit), () => void stops++),
  };
  const sessions = new PassthroughSessions(backend, (event) => emitted.push(event));
  const send = (method: string, sessionId?: string) =>
    sessions.send({ id: 1, method, params: {}, sessionId }, () => 'tab');
  return { sessions, sent, emitted, send, fire: (...a: Parameters<typeof fire>) => fire(...a), stops: () => stops };
}
test('only standard methods on a page session pass through', () => {
  const { sessions } = fixture();
  assert.equal(sessions.handles({ id: 1, method: 'Page.enable', params: {}, sessionId: 's' }, true), true);
  assert.equal(sessions.handles({ id: 1, method: 'Page.enable', params: {} }, false), false);
  assert.equal(sessions.handles({ id: 1, method: 'Oya.analyze', params: {}, sessionId: 's' }, true), false);
  assert.equal(sessions.handles({ id: 1, method: 'Browser.close', params: {}, sessionId: 's' }, true), false);
});
test("a page session's events arrive under that session", async () => {
  const f = fixture();
  await f.send('Page.enable', 's');
  f.fire('Page.loadEventFired', { timestamp: 1 }, '');
  assert.deepEqual(f.emitted, [{ method: 'Page.loadEventFired', params: { timestamp: 1 }, sessionId: 's' }]);
});
test('a child session the tab attaches routes its commands and events to that child', async () => {
  const f = fixture();
  await f.send('Target.setAutoAttach', 's');
  f.fire('Target.attachedToTarget', { sessionId: 'child', targetInfo: {} }, '');
  const child = f.emitted.at(-1).params.sessionId;
  assert.equal(f.sessions.handles({ id: 1, method: 'Runtime.enable', params: {}, sessionId: child }, false), true);
  await f.send('Runtime.enable', child);
  assert.deepEqual(f.sent.at(-1), ['tab', 'Runtime.enable', {}, 'child']);
  f.fire('Runtime.executionContextCreated', { context: {} }, 'child');
  assert.equal(f.emitted.at(-1).sessionId, child);
});
test('events from a child session this connection never saw are dropped', async () => {
  const f = fixture();
  await f.send('Page.enable', 's');
  f.fire('Runtime.consoleAPICalled', {}, 'someone-else');
  assert.equal(f.emitted.length, 0);
});
test('releasing a page session stops its events and forgets its children', async () => {
  const f = fixture();
  await f.send('Page.enable', 's');
  f.fire('Target.attachedToTarget', { sessionId: 'child' }, '');
  const child = f.emitted.at(-1).params.sessionId;
  f.sessions.release('s');
  assert.equal(f.stops(), 1);
  assert.equal(f.sessions.handles({ id: 1, method: 'Runtime.enable', params: {}, sessionId: child }, false), false);
});
