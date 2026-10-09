/**
 * Unit tests for the server message map (src/main/connection/server-messages.ts): auth_ok's order of work, control
 * and cookie messages, the CDP relay, and unknown types ignored.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ServerMessages } from '../../../../src/main/connection/server-messages.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

/** Routes one message the way the socket does. */
const handleServerMessage = (ctx, msg) => new ServerMessages(ctx).handle(msg);

describe('server messages', () => {
  let ctx: any, order: any[];
  beforeEach(() => {
    ctx = mainCtx();
    order = [];
    ctx.persona = {
      active: null,
      flushStorage: async () => true,
      ensureLoginState: () => order.push('login'),
      applyServerFingerprint: async (fp, cookies, now) => order.push(['fingerprint', fp.id, cookies.length, now]),
    };
    ctx.cookies = {
      flushCookieChanges: () => order.push('flush'),
      dumpCookies: async () => order.push('dump'),
      applyCookieSync: async (c, clock) => order.push(['sync', c, clock]),
      answerPull: (id) => order.push(['pulled', id]),
    };
    ctx.tabs = { enterBrowsingMode: (url) => order.push(['browse', url]) };
    ctx.shell.browsingMode = false;
    ctx.socket.ready = false;
  });

  it('applies the persona before going online, then shares cookies', async () => {
    await handleServerMessage(ctx, {
      type: 'auth_ok',
      browser_id: 'srv',
      fingerprint: { id: 'p' },
      control: { mode: 'agent' },
      persona: { name: 'Work' },
      now: 9000,
    });
    const browse = ['browse', 'oya:home'];
    assert.deepEqual(order, ['login', ['fingerprint', 'p', 0, 9000], browse, 'flush', 'dump']);
    assert.equal(ctx.socket.browserId, 'srv');
    assert.equal(ctx.socket.ready, true);
    assert.equal(ctx.config.values.profileName, 'Work');
    assert.deepEqual(ctx.socket.sent.at(-1), { type: 'profile_flush' });
    assert.equal(ctx.socket.pinging, true);
  });

  it('awaits native storage readiness before opening tabs or marking the socket ready', async () => {
    let done;
    ctx.persona.ensureLoginState = () =>
      new Promise((resolve) => {
        done = resolve;
      });
    const auth = handleServerMessage(ctx, { type: 'auth_ok', fingerprint: { id: 'p' } });
    assert.equal(ctx.socket.ready, false);
    assert.deepEqual(order, []);
    done();
    await auth;
    assert.equal(ctx.socket.ready, true);
  });

  it('refuses a save acknowledgment request if native storage could not be delivered', async () => {
    ctx.persona.flushStorage = async () => false;
    await assert.rejects(handleServerMessage(ctx, { type: 'auth_ok' }), /storage could not be sent/);
    assert.deepEqual(ctx.socket.ofType('profile_flush'), []);
  });

  it('answers pings and counts pongs as life', async () => {
    await handleServerMessage(ctx, { type: 'ping' });
    await handleServerMessage(ctx, { type: 'pong' });
    assert.deepEqual(ctx.socket.ofType('pong'), [{ type: 'pong' }]);
    assert.equal(ctx.socket.heardCount, 2);
  });

  it('applies a cookie sync and answers the pull it was for', async () => {
    await handleServerMessage(ctx, { type: 'cookie_sync', cookies: [], pullId: 'p1', now: 9000 });
    assert.deepEqual(order, [
      ['sync', [], { now: 9000, pullId: 'p1' }],
      ['pulled', 'p1'],
    ]);
  });

  it('does not acknowledge a pull whose native cookie restore failed', async () => {
    ctx.cookies.applyCookieSync = async () => {
      throw new Error('Cookie sync incomplete');
    };
    await assert.rejects(handleServerMessage(ctx, { type: 'cookie_sync', cookies: [], pullId: 'p1' }), /incomplete/);
    assert.deepEqual(order, []);
  });

  it('does not go online or share a jar when restoring the persona fails', async () => {
    ctx.persona.applyServerFingerprint = async () => {
      throw new Error('Cookie sync incomplete');
    };
    await assert.rejects(handleServerMessage(ctx, { type: 'auth_ok', fingerprint: { id: 'p' } }), /incomplete/);
    assert.equal(ctx.socket.ready, false);
    assert.deepEqual(order, ['login']);
  });

  it('takes the control state and sets the egress mode from it', async () => {
    const setMode = mock.method(ctx.governance, 'setMode', () => {});
    ctx.control.state = { mode: 'human' };
    await handleServerMessage(ctx, { type: 'control_mode', state: { mode: 'human' } });
    await handleServerMessage(ctx, { type: 'control_mode', mode: 'agent' });
    assert.deepEqual(
      setMode.mock.calls.map((c) => c.arguments[0]),
      ['human', 'agent'],
    );
    setMode.mock.restore();
  });

  it('relays CDP frames and closes relays the server closed', async () => {
    const sock = {
      frames: [],
      send(f) {
        this.frames.push(f);
      },
      close() {
        this.closed = true;
      },
    };
    ctx.relay = { cdpRelays: new Map([['s1', sock]]), openCdpRelay: (sid) => order.push(['open', sid]) };
    await handleServerMessage(ctx, { type: 'cdp_open', sid: 's2' });
    await handleServerMessage(ctx, { type: 'cdp', sid: 's1', data: 7 });
    await handleServerMessage(ctx, { type: 'cdp_close', sid: 's1' });
    assert.deepEqual(sock.frames, ['7']);
    assert.equal(sock.closed, true);
    assert.equal(ctx.relay.cdpRelays.size, 0);
    assert.deepEqual(order, [['open', 's2']]);
  });

  it('does not wait for a command to finish', async () => {
    ctx.commands = { handleCommand: () => new Promise(() => {}) };
    assert.equal(await handleServerMessage(ctx, { type: 'cmd', id: 'c' }), undefined);
  });

  it('remembers when a Sync now was confirmed and how many sites the server keeps', () => {
    mock.timers.enable({ apis: ['Date'], now: 5000 });
    handleServerMessage(ctx, { type: 'profile_saved', sites: ['a.test', 'b.test'] });
    assert.deepEqual(ctx.config.values.lastSync, { at: 5000, sites: 2 });
    handleServerMessage(ctx, { type: 'profile_saved', error: 'Could not save profile. Try again.' });
    assert.deepEqual(ctx.config.values.lastSync, { at: 5000, sites: 2 }, 'a failed save changes nothing');
    assert.equal(ctx.shell.sentOn('profile-saved').length, 2);
    mock.timers.reset();
  });

  it('ignores unknown and inherited message types', async () => {
    assert.equal(await handleServerMessage(ctx, { type: 'nope' }), undefined);
    assert.equal(await handleServerMessage(ctx, { type: 'toString' }), undefined);
  });

  it('forwards the agent’s live events to the shell, so the panel can show the run as it goes', () => {
    const event = { kind: 'step', tool: 'navigate', line: 'Opening amazon.com' };
    handleServerMessage(ctx, { type: 'agent-event', runId: 'r1', event });
    assert.deepEqual(ctx.shell.sentOn('agent-event'), [{ runId: 'r1', event }]);
  });
});
