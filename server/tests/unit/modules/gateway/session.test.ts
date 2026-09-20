/**
 * Unit tests for a gateway Session: it relays browser traffic to its client,
 * holds messages while the client is away, survives a disconnect for the
 * grace period, and is destroyed exactly once.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-session-');
const { Session } = await import('../../../../src/modules/gateway/session.ts');
const { sessions } = await import('../../../../src/modules/gateway/session-store.ts');
const { GRACE_MS, MAX_PENDING_TO_CLIENT } = await import('../../../../src/modules/gateway/constants.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');
const { stubControl, FakeWs } = await import('../../support/gateway.ts');
const { advance } = await import('../../support/http.ts');

/** A session over a fake browser socket, registered in the store. */
function open(id = 'sess-1') {
  const upstream = new FakeWs();
  const release = mock.fn(async () => {});
  const s = new Session({ id, apiKey: 'key-a', provider: 'chrome', release, upstream, profile: null });
  s.bindUpstream();
  sessions.set(id, s);
  return { s, upstream, release };
}

beforeEach(() => {
  stubControl();
  mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 1_000_000 });
});
afterEach(() => {
  mock.timers.reset();
  mock.restoreAll();
  sessions.clear();
});

describe('Session', () => {
  it('is owned by the fingerprint of its key, never the key itself', () => {
    const { s } = open();
    assert.equal(s.owner, fingerprint('key-a'));
  });

  it('relays browser messages to the connected client and counts the bytes', () => {
    const { s, upstream } = open();
    const client = new FakeWs();
    s.attach(client);
    upstream.emit('message', Buffer.from('{"method":"x"}'), false);
    assert.deepEqual(client.json(), [{ method: 'x' }]);
    assert.equal(s.bytesDown, 14);
  });

  it('holds browser messages while no client is connected and delivers them on attach', () => {
    const { s, upstream } = open();
    upstream.emit('message', Buffer.from('{"n":1}'), false);
    upstream.emit('message', Buffer.from('{"n":2}'), false);
    const client = new FakeWs();
    s.attach(client);
    assert.deepEqual(client.json(), [{ n: 1 }, { n: 2 }]);
    assert.equal(s.pendingToClient.length, 0);
  });

  it('holds at most the pending limit', () => {
    const { s, upstream } = open();
    for (let i = 0; i < MAX_PENDING_TO_CLIENT + 5; i++) upstream.emit('message', Buffer.from('{}'), false);
    assert.equal(s.pendingToClient.length, MAX_PENDING_TO_CLIENT);
  });

  it('forwards client commands to the browser', async () => {
    const { s, upstream } = open();
    const client = new FakeWs();
    s.attach(client);
    client.emit('message', Buffer.from('{"id":1,"method":"Page.navigate"}'), false);
    await advance(0);
    assert.deepEqual(upstream.json(), [{ id: 1, method: 'Page.navigate' }]);
  });

  it('releases a command slot when the browser replies', async () => {
    const { s, upstream } = open();
    const client = new FakeWs();
    s.attach(client);
    client.emit('message', Buffer.from('{"id":1,"method":"Page.navigate"}'), false);
    await advance(0);
    upstream.emit('message', Buffer.from('{"id":1,"result":{}}'), false);
    assert.equal(s.commands.releases.size, 0);
  });

  it('survives a client disconnect for the grace period, then ends', async () => {
    const { s, release } = open();
    const client = new FakeWs();
    s.attach(client);
    client.emit('close');
    assert.equal(s.client, null);
    mock.timers.tick(GRACE_MS - 1);
    assert.equal(s.closed, false);
    mock.timers.tick(1);
    await advance(0);
    assert.equal(s.closed, true);
    assert.equal(release.mock.callCount(), 1);
    assert.equal(sessions.has('sess-1'), false);
  });

  it('keeps going when a client reconnects within the grace period', async () => {
    const { s } = open();
    const first = new FakeWs();
    s.attach(first);
    first.emit('close');
    s.attach(new FakeWs());
    mock.timers.tick(GRACE_MS * 2);
    await advance(0);
    assert.equal(s.closed, false);
  });

  it('ignores the close of a client it has already replaced', () => {
    const { s } = open();
    const old = new FakeWs();
    s.attach(old);
    const current = new FakeWs();
    s.attach(current);
    old.emit('close');
    assert.equal(s.client, current);
  });

  it('ends when the browser closes or errors', async () => {
    const closing = open('sess-close');
    closing.upstream.emit('close');
    const failing = open('sess-error');
    failing.upstream.emit('error', new Error('boom'));
    await advance(0);
    assert.deepEqual([closing.s.closed, failing.s.closed], [true, true]);
  });

  it('is destroyed only once, however often it is asked', async () => {
    const { s, release } = open();
    await s.destroy('first');
    await s.destroy('second');
    assert.equal(release.mock.callCount(), 1);
  });

  it('describes itself with its traffic and whether a client is on it', () => {
    const { s } = open();
    s.bytesUp = 3;
    mock.timers.tick(2600);
    assert.deepEqual(s.toJSON(), {
      id: 'sess-1',
      provider: 'chrome',
      profile: null,
      attachedTo: null,
      connected: false,
      startedAt: new Date(1_000_000).toISOString(),
      seconds: 3,
      bytesUp: 3,
      bytesDown: 0,
      recording: false,
    });
  });
});
