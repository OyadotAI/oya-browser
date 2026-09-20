/**
 * Unit tests for ending a gateway session: command slots are released, the
 * attachment record dropped, sockets closed, the browser handed back and the
 * lifecycle, usage and audit recorded.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-teardown-');
const { Session } = await import('../../../../src/modules/gateway/session.ts');
const { sessions } = await import('../../../../src/modules/gateway/session-store.ts');
const { teardown } = await import('../../../../src/modules/gateway/session-teardown.ts');
const { CloseCode } = await import('../../../../src/modules/gateway/constants.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const usage = await import('../../../../src/platform/usage.ts');
const { recent } = await import('../../../../src/platform/audit.ts');
const { stubControl, FakeWs } = await import('../../support/gateway.ts');

/** A live session with a client, as the store holds it. */
function live(id: string, extra: Record<string, any> = {}) {
  const s = new Session({
    id,
    apiKey: `key-${id}`,
    provider: 'chrome',
    release: mock.fn(async () => {}),
    upstream: new FakeWs(),
    profile: null,
  });
  Object.assign(s, extra);
  s.client = new FakeWs();
  s.closed = true;
  sessions.set(id, s);
  return s;
}

let control$: ReturnType<typeof stubControl>;
beforeEach(() => {
  control$ = stubControl();
  mock.method(console, 'error', () => {});
});
afterEach(() => {
  mock.restoreAll();
  sessions.clear();
});

describe('teardown', () => {
  it('closes the client and browser, and forgets the session', async () => {
    const s = live('t-1');
    await teardown(s, 'done');
    assert.deepEqual(s.client.closed, { code: CloseCode.GOING_AWAY, reason: 'done' });
    assert.ok(s.upstream.closed);
    assert.equal(sessions.has('t-1'), false);
  });

  it('marks a provider session cleanup_pending, hands the browser back, then marks it stopped', async () => {
    const s = live('t-2');
    await teardown(s, 'done');
    assert.equal(s.release.mock.callCount(), 1);
    assert.deepEqual(
      control$.of('update').map((c) => c.args[2].state),
      ['cleanup_pending', 'stopped'],
    );
  });

  it('leaves the session cleanup_pending when handing the browser back fails', async () => {
    const s = live('t-3', { release: async () => Promise.reject(new Error('vendor down')) });
    await teardown(s, 'done');
    assert.deepEqual(
      control$.of('update').map((c) => c.args[2].state),
      ['cleanup_pending'],
    );
    assert.match((console.error as any).mock.calls[0].arguments.join(' '), /cleanup pending: vendor down/);
  });

  it('drops the attachment record of a fleet browser and books no browser seconds for it', async () => {
    const s = live('t-4', { attachedTo: 'b-fleet' });
    await control().store.transact(async (tx) => {
      await tx.get('attachment', 't-4');
      tx.put('attachment', 't-4', { id: 't-4', browserId: 'b-fleet' });
    });
    s.startedAt = Date.now() - 5000;
    await teardown(s, 'done');
    assert.equal(await control().store.get('attachment', 't-4'), null);
    assert.equal(control$.of('update').length, 0);
    assert.equal(usage.current('key-t-4').browser_seconds, 0);
  });

  it('books usage and audits the end with its reason', async () => {
    const s = live('t-5');
    s.bytesDown = 42;
    s.startedAt = Date.now() - 3000;
    await teardown(s, 'client left');
    assert.equal(usage.current('key-t-5').bytes_out, 42);
    assert.equal(usage.current('key-t-5').browser_seconds, 3);
    const ended = recent({ action: 'gateway.session.end' }).find((r) => r.target_id === 't-5');
    assert.deepEqual(ended.meta, { provider: 'chrome', seconds: 3, reason: 'client left', profile: null });
  });

  it('releases the command slots still held', async () => {
    const s = live('t-6');
    const finish = mock.fn(async () => {});
    s.commands.releases.set(':1', finish);
    await teardown(s, 'done');
    assert.equal(finish.mock.callCount(), 1);
    assert.equal(s.commands.releases.size, 0);
  });

  it('closes the connection a profile restore held open, and logs a failed capture', async () => {
    const profileConn = { close: mock.fn() };
    const s = live('t-7', { profile: 'bad name!', profileConn });
    await teardown(s, 'done');
    assert.equal(profileConn.close.mock.callCount(), 1);
    assert.match((console.error as any).mock.calls[0].arguments.join(' '), /profile capture failed for bad name!/);
  });

  it('survives sockets that throw on close', async () => {
    const s = live('t-8');
    s.client.close = () => {
      throw new Error('already closed');
    };
    s.upstream.close = s.client.close;
    await assert.doesNotReject(teardown(s, 'done'));
  });
});
