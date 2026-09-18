/**
 * Unit tests for CommandGate: client commands reach the browser one at a
 * time and in order, each holding a control-plane command slot until its
 * reply (or until it is stuck), and bad commands close the client.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-commands-');
const { CommandGate } = await import('../../../../src/modules/gateway/session-commands.ts');
const { CloseCode, STUCK_COMMAND_MS } = await import('../../../../src/modules/gateway/constants.ts');
const { stubControl, FakeWs } = await import('../../support/gateway.ts');
const { advance } = await import('../../support/http.ts');

/** A session with a connected client and an open browser socket. */
function gated(extra: object = {}) {
  const client = new FakeWs();
  const upstream = new FakeWs();
  const session = { id: 'sess-1', closed: false, client, upstream, bytesUp: 0, ...extra };
  return { gate: new CommandGate(session), session, client, upstream };
}
/** A CDP command as the client sends it. */
const cmd = (id: any, extra: object = {}) => Buffer.from(JSON.stringify({ id, method: 'Page.navigate', ...extra }));
/** Lets queued commands run. */
const flush = () => advance(0);

let control$: ReturnType<typeof stubControl>;
beforeEach(() => {
  control$ = stubControl();
  mock.timers.enable({ apis: ['setTimeout'] });
});
afterEach(() => {
  mock.timers.reset();
  mock.restoreAll();
});

describe('CommandGate', () => {
  it('forwards commands to the browser in order, counting their bytes', async () => {
    const { gate, client, upstream, session } = gated();
    gate.enqueue(client, cmd(1), false);
    gate.enqueue(client, cmd(2), false);
    await flush();
    assert.deepEqual(
      upstream.json().map((m) => m.id),
      [1, 2],
    );
    assert.equal(session.bytesUp, cmd(1).length * 2);
  });

  it('takes a command slot against the attached browser, else the session', async () => {
    const own = gated();
    own.gate.enqueue(own.client, cmd(1), false);
    const attached = gated({ attachedTo: 'b-fleet' });
    attached.gate.enqueue(attached.client, cmd(1), false);
    await flush();
    assert.deepEqual(
      control$.of('beginCommand').map((c) => c.args[0]),
      ['sess-1', 'b-fleet'],
    );
  });

  it('releases the slot when the browser replies, keyed by CDP session and id', async () => {
    const { gate, client } = gated();
    gate.enqueue(client, cmd(7, { sessionId: 'S' }), false);
    await flush();
    assert.deepEqual([...gate.releases.keys()], ['S:7']);
    gate.settleReply(Buffer.from(JSON.stringify({ id: 7, sessionId: 'S', result: {} })));
    assert.equal(gate.releases.size, 0);
  });

  it('ignores events and unparseable browser messages', async () => {
    const { gate, client } = gated();
    gate.enqueue(client, cmd(1), false);
    await flush();
    gate.settleReply(Buffer.from('{"method":"Page.loadEventFired"}'));
    gate.settleReply(Buffer.from('not json'));
    assert.equal(gate.releases.size, 1);
  });

  it('releases a command the browser never answers once it is stuck', async () => {
    const { gate, client } = gated();
    gate.enqueue(client, cmd(1), false);
    await flush();
    mock.timers.tick(STUCK_COMMAND_MS);
    assert.equal(gate.releases.size, 0);
  });

  it('closes the client for a command without an id', async () => {
    const { gate, client, upstream } = gated();
    gate.enqueue(client, Buffer.from('{"method":"Page.navigate"}'), false);
    await flush();
    assert.equal(client.closed.code, CloseCode.POLICY_VIOLATION);
    assert.equal(upstream.sent.length, 0);
  });

  it('closes the client for a duplicate command id still in flight', async () => {
    const { gate, client } = gated();
    gate.enqueue(client, cmd(1), false);
    gate.enqueue(client, cmd(1), false);
    await flush();
    assert.equal(client.closed.reason, 'Session access paused, revoked, or command invalid');
  });

  it('closes the client when the control plane refuses the command', async () => {
    mock.method(
      (await import('../../../../src/modules/control/service.ts')).control() as any,
      'beginCommand',
      async () => Promise.reject(new Error('paused')),
    );
    const { gate, client } = gated();
    gate.enqueue(client, cmd(1), false);
    await flush();
    assert.equal(client.closed.code, CloseCode.POLICY_VIOLATION);
  });

  it('gives the slot back and closes the client when the browser has gone', async () => {
    const { gate, client, upstream } = gated();
    upstream.readyState = 3;
    gate.enqueue(client, cmd(1), false);
    await flush();
    assert.equal(gate.releases.size, 0);
    assert.equal(client.closed.code, CloseCode.POLICY_VIOLATION);
  });

  it('drops commands from a client that is no longer the session’s, or after it closed', async () => {
    const { gate, upstream, session } = gated();
    gate.enqueue(new FakeWs(), cmd(1), false);
    await flush();
    session.closed = true;
    gate.enqueue(session.client, cmd(2), false);
    await flush();
    assert.equal(upstream.sent.length, 0);
  });

  it('releases every slot still held when the session ends', async () => {
    const { gate, client } = gated();
    gate.enqueue(client, cmd(1), false);
    gate.enqueue(client, cmd(2), false);
    await flush();
    await gate.releaseAll();
    assert.equal(gate.releases.size, 0);
  });
});
