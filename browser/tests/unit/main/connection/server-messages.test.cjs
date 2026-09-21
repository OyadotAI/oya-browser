/**
 * Unit tests for the server message map: auth_ok's order of work, control
 * and cookie messages, the CDP relay, and unknown types ignored.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { handleServerMessage } = require('../../../../main/connection/server-messages.cjs');
const governance = require('../../../../governance');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('server messages', () => {
  let ctx, order;
  beforeEach(() => {
    ctx = mainCtx();
    order = [];
    ctx.persona = {
      active: null,
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
    const browse = ['browse', 'https://google.com'];
    assert.deepEqual(order, ['login', ['fingerprint', 'p', 0, 9000], browse, 'flush', 'dump']);
    assert.equal(ctx.socket.browserId, 'srv');
    assert.equal(ctx.socket.ready, true);
    assert.equal(ctx.config.values.profileName, 'Work');
    assert.deepEqual(ctx.socket.sent.at(-1), { type: 'profile_flush' });
    assert.equal(ctx.socket.pinging, true);
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

  it('takes the control state and sets the egress mode from it', async () => {
    const setMode = mock.method(governance, 'setMode', () => {});
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

  it('ignores unknown and inherited message types', async () => {
    assert.equal(await handleServerMessage(ctx, { type: 'nope' }), undefined);
    assert.equal(await handleServerMessage(ctx, { type: 'toString' }), undefined);
  });
});
