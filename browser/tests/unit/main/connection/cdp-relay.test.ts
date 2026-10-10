/** Remote gateway frames terminate in native operations without a debugging port or outbound socket. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { CdpRelay } from '../../../../src/main/connection/cdp-relay.ts';
import { flush } from '../../support/fakes.cjs';

/** Independent native resources with observable execution and disposal. */
function fixture() {
  const sent: any[] = [],
    executed: any[] = [],
    disposed: number[] = [];
  let allowed = true,
    connections = 0;
  const backend: any = {
    connection: () => {
      const id = ++connections;
      return { ...backend, connection: undefined, dispose: () => disposed.push(id) };
    },
    targets: () => [{ targetId: 'tab', type: 'page', title: 'Private', url: 'https://example.test' }],
    execute: async (...args: any[]) => {
      executed.push(args);
      return { value: 'native' };
    },
  };
  const relay = new CdpRelay({ backend, allowed: () => allowed, socket: { send: (m) => sent.push(m) } });
  return {
    relay,
    sent,
    executed,
    disposed,
    revoke: () => {
      allowed = false;
    },
  };
}

/** Await asynchronous dispatch and read the last reply for one connection. */
async function call(f: ReturnType<typeof fixture>, sid: string, message: object) {
  f.relay.cdpRelays.get(sid)!.send(JSON.stringify(message));
  await flush();
  return f.sent
    .filter((m) => m.type === 'cdp' && m.sid === sid)
    .map((m) => JSON.parse(m.data))
    .at(-1);
}

describe('native CdpRelay', () => {
  it('opens without a debugging port and dispatches native capabilities', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    assert.deepEqual(f.sent, [{ type: 'cdp_opened', sid: 'a' }]);
    assert.equal((await call(f, 'a', { id: 1, method: 'Oya.getCapabilities' })).result.compatibility, 'partial');
    f.relay.closeCdpRelays();
  });
  it('refuses unauthenticated opens before allocating resources', async () => {
    const f = fixture();
    f.revoke();
    await f.relay.openCdpRelay('a');
    assert.equal(f.sent[0].type, 'cdp_closed');
    assert.equal(f.relay.cdpRelays.size, 0);
    assert.deepEqual(f.disposed, []);
  });
  it('isolates attached target handles between remote sessions', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    await f.relay.openCdpRelay('b');
    const attached = await call(f, 'a', {
      id: 1,
      method: 'Target.attachToTarget',
      params: { targetId: 'tab', flatten: true },
    });
    const sessionId = attached.result.sessionId;
    assert.ok(
      (await call(f, 'b', { id: 2, sessionId, method: 'Runtime.evaluate', params: { expression: '1' } })).error,
    );
    assert.equal(f.executed.length, 0);
    assert.deepEqual(
      (await call(f, 'a', { id: 3, sessionId, method: 'Runtime.evaluate', params: { expression: '1' } })).result,
      { value: 'native' },
    );
    f.relay.closeCdpRelays();
    assert.deepEqual(f.disposed, [1, 2]);
  });
  it('rejects unsupported methods and remote filesystem destinations', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    for (const method of ['Browser.close', 'Target.exposeDevToolsProtocol', 'Browser.setDownloadBehavior'])
      assert.ok((await call(f, 'a', { id: 1, method })).error);
    assert.equal(f.executed.length, 0);
    f.relay.closeCdpRelays();
  });
  it('rechecks takeover after enqueueing', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    const pending = call(f, 'a', { id: 1, method: 'Target.getTargets' });
    f.revoke();
    assert.match((await pending).error.message, /unavailable/);
    f.relay.closeCdpRelays();
  });
  it('duplicate opens preserve the original connection and handles', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    const original = f.relay.cdpRelays.get('a');
    await f.relay.openCdpRelay('a');
    assert.equal(f.relay.cdpRelays.get('a'), original);
    assert.equal(f.sent.length, 1);
    f.relay.closeCdpRelays();
    assert.deepEqual(f.disposed, [1]);
  });
  it('disconnect discards queued commands and disposes once', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    f.relay.cdpRelays.get('a')!.send(JSON.stringify({ id: 1, method: 'Target.getTargets' }));
    f.relay.closeCdpRelays();
    f.relay.closeCdpRelays();
    await flush();
    assert.equal(f.sent.filter((m) => m.type === 'cdp').length, 0);
    assert.deepEqual(f.disposed, [1]);
  });
  it('rejects malformed input without dispatch', async () => {
    const f = fixture();
    await f.relay.openCdpRelay('a');
    f.relay.cdpRelays.get('a')!.send('not json');
    const reply = JSON.parse(f.sent.at(-1).data);
    assert.ok(reply.error);
    assert.equal(reply.id, null);
    f.relay.closeCdpRelays();
  });
});
