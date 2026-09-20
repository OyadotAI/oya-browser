/**
 * Unit tests for control-state.cjs: who may drive the browser, offline and
 * connected, through takeover, return, renewal, expiry and disconnects. The
 * server is a fake that answers each desktop_control request.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { createControlState } = require('../../control-state.cjs');
const { flush } = require('./support/fakes.cjs');
const {
  CONTROL_REQUEST_TIMEOUT_MS,
  CONTROL_RENEW_BEFORE_MS,
  CONTROL_TICK_MS,
  LOCAL_DRAIN_TIMEOUT_MS,
} = require('../../constants.cjs');

/** Server modes after each handoff action. */
const MODES = { request: 'paused', acquire: 'human', return: 'agent', renew: 'human' };

/**
 * A control state wired to a fake server. `answer(message)` may be replaced to
 * change how the server replies; returning undefined leaves a request pending.
 */
function withServer({ up = true } = {}) {
  const sent = [];
  const snapshots = [];
  const server = { revision: 1, up };
  let control;
  server.answer = (message) => {
    if (message.action === 'command-start') return { id: message.id, token: 'slot' };
    if (message.action === 'command-end') return { id: message.id };
    const mode = MODES[message.action];
    server.revision++;
    const state = { mode, mine: mode !== 'agent', expiresAt: Date.now() + 300_000, revision: server.revision };
    return { id: message.id, state };
  };
  control = createControlState({
    changed: (snapshot) => snapshots.push(snapshot),
    send: (message) => {
      sent.push(message);
      const reply = server.answer(message);
      if (reply) queueMicrotask(() => control.result(reply));
      return server.up;
    },
  });
  return { control, sent, snapshots, server };
}

describe('createControlState', () => {
  let ctx;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout', 'Date'], now: 1_000_000 });
    ctx = withServer();
  });
  afterEach(() => {
    ctx.control.dispose();
    mock.timers.reset();
  });

  it('lets a person interact when nothing automates the offline browser', () => {
    assert.equal(ctx.control.snapshot().interactive, true);
    assert.equal(ctx.control.snapshot().mode, 'offline');
  });

  it('gives an offline browser to a local CDP client, and back when it leaves', () => {
    ctx.control.localClient(1);
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().interactive], ['agent', false]);
    ctx.control.localClient(-1);
    ctx.control.localClient(-1);
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().localClients], ['offline', 0]);
  });

  it('shows a running local command as agent control', async () => {
    const finish = await ctx.control.beginLocalCommand();
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().mine], ['agent', false]);
    finish();
    finish();
    assert.equal(ctx.control.snapshot().interactive, true, 'ending twice counts once');
  });

  it('takes local control, pausing new automation until it is returned', async () => {
    ctx.control.localClient(1);
    const state = await ctx.control.change('acquire');
    assert.deepEqual([state.mode, state.mine, state.interactive], ['human', true, true]);
    await assert.rejects(ctx.control.beginLocalCommand(), /paused for human control/);
    const back = await ctx.control.change('return');
    assert.deepEqual([back.mode, back.interactive], ['agent', false]);
  });

  it('waits for a running local command before a takeover completes', async () => {
    const finish = await ctx.control.beginLocalCommand();
    const taking = ctx.control.change('acquire');
    mock.timers.tick(100);
    finish();
    mock.timers.tick(100);
    assert.equal((await taking).mode, 'human');
  });

  it('fails a takeover when a local command outlasts the drain', async () => {
    await ctx.control.beginLocalCommand();
    const refused = assert.rejects(ctx.control.change('acquire'), /still running/);
    for (let waited = 0; waited <= LOCAL_DRAIN_TIMEOUT_MS; waited += 50) {
      mock.timers.tick(50);
      await flush();
    }
    await refused;
    assert.equal(ctx.control.snapshot().busy, false);
  });

  it('refuses an unknown action and a second handoff', async () => {
    await assert.rejects(ctx.control.change('steal'), /Invalid control action/);
    const first = ctx.control.change('acquire');
    await assert.rejects(ctx.control.change('return'), /already in progress/);
    await first;
  });

  it('asks the server to drain and then hand over when connected', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    const state = await ctx.control.change('acquire');
    assert.deepEqual(
      ctx.sent.map((m) => m.action),
      ['request', 'acquire'],
    );
    assert.deepEqual([state.mode, state.mine, state.interactive], ['human', true, true]);
  });

  it('refuses control on a server that does not support it', async () => {
    ctx.control.connect(null);
    assert.equal(ctx.control.snapshot().mode, 'unavailable');
    await assert.rejects(ctx.control.change('acquire'), /does not support/);
  });

  it('ignores a server state older than the one it has', () => {
    ctx.control.connect({ mode: 'human', mine: true, revision: 5, expiresAt: Date.now() + 1000 });
    ctx.control.receive({ mode: 'agent', revision: 4 });
    ctx.control.receive(null);
    assert.equal(ctx.control.snapshot().mode, 'human');
  });

  it('holds a command slot on the server while a local command runs', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    const finish = await ctx.control.beginLocalCommand();
    finish();
    assert.deepEqual(ctx.sent.at(-1), {
      type: 'desktop_control',
      id: ctx.sent.at(-1).id,
      action: 'command-end',
      token: 'slot',
    });
  });

  it('undoes a local command the server refuses a slot for', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    ctx.server.answer = (message) => ({ id: message.id, error: 'Operator has control' });
    await assert.rejects(ctx.control.beginLocalCommand(), /Operator has control/);
    ctx.server.answer = () => undefined;
    ctx.control.disconnect();
    assert.equal(ctx.control.snapshot().mode, 'disconnected');
  });

  it('releases the slot of a command-start answered after it stopped waiting', async () => {
    ctx.control.result({ id: 'gone', token: 'late' });
    assert.deepEqual(ctx.sent.at(-1).token, 'late');
    assert.equal(ctx.sent.at(-1).action, 'command-end');
  });

  it('fails a request the server never answers', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    ctx.server.answer = () => undefined;
    const taking = ctx.control.change('acquire');
    mock.timers.tick(CONTROL_REQUEST_TIMEOUT_MS);
    await assert.rejects(taking, /timed out/);
  });

  it('fails at once when the socket is down', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    ctx.server.up = false;
    ctx.server.answer = () => undefined;
    await assert.rejects(ctx.control.change('acquire'), /Browser is disconnected/);
  });

  it('fails pending requests when the socket closes', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    ctx.server.answer = () => undefined;
    const taking = ctx.control.change('acquire');
    ctx.control.disconnect();
    await assert.rejects(taking, /disconnected during handoff/);
  });

  it('keeps local automation paused when the socket drops during human control', async () => {
    ctx.control.localClient(1);
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    await ctx.control.change('acquire');
    ctx.control.disconnect();
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().mine], ['paused', true]);
    await assert.rejects(ctx.control.beginLocalCommand(), /paused/);
  });

  it('renews human control before it runs out', async () => {
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + CONTROL_RENEW_BEFORE_MS });
    mock.timers.tick(CONTROL_TICK_MS);
    await flush();
    assert.equal(ctx.sent.at(-1).action, 'renew');
    assert.ok(ctx.control.snapshot().expiresAt > Date.now() + CONTROL_RENEW_BEFORE_MS);
  });

  it('pauses when human control expires', () => {
    ctx.server.answer = () => undefined;
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + 500 });
    mock.timers.tick(CONTROL_TICK_MS);
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().interactive], ['paused', false]);
  });

  it('publishes every change to the listener', async () => {
    const before = ctx.snapshots.length;
    await ctx.control.change('acquire');
    assert.ok(ctx.snapshots.length > before);
    assert.equal(ctx.snapshots.at(-1).mode, 'human');
  });
});
