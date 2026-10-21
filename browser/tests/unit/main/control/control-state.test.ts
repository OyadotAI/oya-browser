/**
 * Unit tests for src/main/control/control-state.ts: who may drive the browser, offline and
 * connected, through takeover, return, renewal, expiry and disconnects. The
 * server is a fake that answers each desktop_control request.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { DesktopControl, drivenElsewhere } from '../../../../src/main/control/control-state.ts';
import { flush } from '../../support/fakes.cjs';
import {
  CONTROL_REQUEST_TIMEOUT_MS,
  CONTROL_RENEW_BEFORE_MS,
  CONTROL_TICK_MS,
  LOCAL_DRAIN_TIMEOUT_MS,
} from '../../../../src/main/control/constants.ts';

/** The server's human-control lease, as the fake server grants it. */
const HUMAN_CONTROL_MS = 300_000;

/** Server modes after each handoff action. */
const MODES = { request: 'paused', acquire: 'human', return: 'agent', renew: 'human' };

/**
 * A control state wired to a fake server. `answer(message)` may be replaced to
 * change how the server replies; returning undefined leaves a request pending.
 */
function withServer({ up = true } = {}) {
  const sent = [];
  const snapshots = [];
  const server: any = { revision: 1, up };
  server.answer = (message) => {
    if (message.action === 'command-start') return { id: message.id, token: 'slot' };
    if (message.action === 'command-end') return { id: message.id };
    const mode = MODES[message.action];
    server.revision++;
    const state = { mode, mine: mode !== 'agent', expiresAt: Date.now() + HUMAN_CONTROL_MS, revision: server.revision };
    return { id: message.id, state };
  };
  const control: DesktopControl = new DesktopControl({
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

/** The handoff actions sent to the server so far, command slots left out. */
const actions = (ctx) => ctx.sent.map((m) => m.action).filter((a) => !a.startsWith('command-'));

/** The fake server's default answer, for a test that overrides one action. */
let answer: (message: any) => unknown;

/** A server answer refusing `action` with `error`, pausing the browser with nobody holding it. */
function refuse(ctx, action: string, error: string) {
  return (message) => {
    if (message.action !== action) return undefined;
    const revision = ++ctx.server.revision;
    return { id: message.id, error, state: { mode: 'paused', mine: false, revision } };
  };
}

/** Connects with the agent driving and takes control, as a person pressing Human Mode. */
async function holdConnected(ctx): Promise<void> {
  ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
  await ctx.control.change('acquire');
}

describe('DesktopControl', () => {
  let ctx: any;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout', 'Date'], now: 1_000_000 });
    ctx = withServer();
    answer = ctx.server.answer;
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

  it('takes control back after a reconnect when the person held it as the socket dropped', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    await ctx.control.change('acquire');
    ctx.control.disconnect();
    ctx.snapshots.length = 0;
    ctx.control.connect({ mode: 'agent', mine: false, revision: ctx.server.revision });
    await flush();
    assert.equal(ctx.control.snapshot().mode, 'human');
    assert.equal(ctx.control.snapshot().interactive, true);
    assert.equal(
      ctx.snapshots.some((s) => s.mode === 'agent'),
      false,
    );
  });

  it('takes control back after a reconnect when the socket dropped mid-retake', async () => {
    await holdConnected(ctx);
    ctx.server.answer = () => undefined;
    ctx.control.receive({ mode: 'paused', mine: false, revision: ctx.server.revision + 1 });
    ctx.control.disconnect();
    await flush();
    ctx.server.answer = answer;
    ctx.control.connect({ mode: 'paused', mine: true, revision: ctx.server.revision + 2 });
    await flush();
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().interactive], ['human', true]);
  });

  /** Holds control, then sends a return whose answer never comes, and drops the socket under it. */
  async function dropDuringReturn(ctx): Promise<void> {
    await holdConnected(ctx);
    ctx.server.answer = () => undefined;
    const returning = ctx.control.change('return').catch(() => {});
    ctx.control.disconnect();
    await returning;
    ctx.server.answer = answer;
    ctx.sent.length = 0;
  }

  it('never takes control back after a reconnect when the socket dropped while the person was giving it back', async () => {
    await dropDuringReturn(ctx);
    ctx.control.connect({ mode: 'paused', mine: false, revision: ctx.server.revision + 1 });
    await flush();
    assert.deepEqual(actions(ctx), []);
  });

  it("leaves the agent in control after a reconnect when the person's return was applied but its answer was lost", async () => {
    await dropDuringReturn(ctx);
    ctx.control.connect({ mode: 'agent', mine: false, revision: ctx.server.revision + 1 });
    await flush();
    assert.deepEqual(actions(ctx), []);
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().mine], ['agent', false]);
  });

  it('leaves the agent in control after a reconnect when someone handed it over while the socket was down', async () => {
    await holdConnected(ctx);
    ctx.server.answer = () => undefined;
    ctx.control.receive({ mode: 'paused', mine: false, revision: ctx.server.revision + 1 });
    ctx.control.disconnect();
    await flush();
    ctx.server.answer = answer;
    ctx.sent.length = 0;
    ctx.control.connect({ mode: 'agent', mine: false, revision: ctx.server.revision + 2 });
    await flush();
    assert.deepEqual(actions(ctx), []);
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().mine], ['agent', false]);
  });

  it('takes control back after a reconnect to a new session that says agent with revision 0', async () => {
    await holdConnected(ctx);
    ctx.control.disconnect();
    ctx.sent.length = 0;
    ctx.control.connect({ mode: 'agent', mine: false, revision: 0 });
    await flush();
    assert.deepEqual(actions(ctx), ['request', 'acquire']);
    assert.equal(ctx.control.snapshot().interactive, true);
  });

  it('leaves control with the agent after a reconnect when the person did not hold it', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    ctx.control.disconnect();
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    await flush();
    assert.equal(ctx.control.snapshot().mode, 'agent');
  });

  it('reports the agent in control when taking it back after a reconnect fails', async () => {
    ctx.control.connect({ mode: 'agent', mine: false, revision: 1 });
    await ctx.control.change('acquire');
    ctx.control.disconnect();
    ctx.server.answer = (message) => ({ id: message.id, error: 'control_busy' });
    ctx.control.connect({ mode: 'agent', mine: false, revision: ctx.server.revision });
    await flush();
    assert.equal(ctx.control.snapshot().mode, 'agent');
    assert.equal(ctx.control.snapshot().busy, false);
  });

  it('renews human control before it runs out', async () => {
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + CONTROL_RENEW_BEFORE_MS });
    mock.timers.tick(CONTROL_TICK_MS);
    await flush();
    assert.equal(ctx.sent.at(-1).action, 'renew');
    assert.ok(ctx.control.snapshot().expiresAt > Date.now() + CONTROL_RENEW_BEFORE_MS);
  });

  it('shows a retake as a paused handoff, never the agent, while it waits', () => {
    ctx.server.answer = () => undefined;
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + 500 });
    mock.timers.tick(CONTROL_TICK_MS);
    const { mode, interactive, busyAction } = ctx.control.snapshot();
    assert.deepEqual([mode, interactive, busyAction], ['paused', false, 'acquire']);
  });

  it('takes control back once a held hold lapses, with nobody else driving', async () => {
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + 500 });
    ctx.snapshots.length = 0;
    mock.timers.tick(CONTROL_TICK_MS);
    await flush();
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().interactive], ['human', true]);
    assert.equal(
      ctx.snapshots.some((s) => s.mode === 'agent'),
      false,
    );
  });

  it("takes control back when the server's lease sweep pauses the hold", async () => {
    await holdConnected(ctx);
    ctx.control.receive({ mode: 'paused', mine: false, revision: ctx.server.revision + 1 });
    await flush();
    assert.deepEqual(actions(ctx), ['request', 'acquire', 'request', 'acquire']);
    assert.equal(ctx.control.snapshot().interactive, true);
  });

  it('takes control back when the server refuses a renewal and pauses the hold', async () => {
    const renew = refuse(ctx, 'renew', 'Human control has expired or changed');
    ctx.server.answer = (message) => (message.action === 'renew' ? renew(message) : answer(message));
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + CONTROL_RENEW_BEFORE_MS });
    mock.timers.tick(CONTROL_TICK_MS);
    await flush();
    assert.deepEqual(actions(ctx), ['renew', 'request', 'acquire']);
    assert.equal(ctx.control.snapshot().interactive, true);
  });

  it('stays with the agent when the hold ends in agent control', async () => {
    await holdConnected(ctx);
    ctx.control.receive({ mode: 'agent', mine: false, revision: ctx.server.revision + 1 });
    await flush();
    assert.deepEqual(actions(ctx), ['request', 'acquire']);
    assert.equal(ctx.control.snapshot().mode, 'agent');
  });

  it("stays out of another operator's hold", async () => {
    await holdConnected(ctx);
    ctx.control.receive({ mode: 'human', mine: false, revision: ctx.server.revision + 1, expiresAt: Date.now() + 1 });
    await flush();
    assert.deepEqual(actions(ctx), ['request', 'acquire']);
    assert.equal(ctx.control.snapshot().interactive, false);
  });

  it('asks once per lapse: a refused retake stays paused', async () => {
    ctx.server.answer = refuse(ctx, 'request', 'Another operator has control');
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() + 500 });
    for (let i = 0; i < 10; i++) {
      mock.timers.tick(CONTROL_TICK_MS);
      await flush();
    }
    assert.deepEqual(actions(ctx), ['request']);
    assert.deepEqual([ctx.control.snapshot().mode, ctx.control.snapshot().busy], ['paused', false]);
  });

  it('never retakes a hold the person was giving back, even when the return is refused', async () => {
    await holdConnected(ctx);
    ctx.server.answer = refuse(ctx, 'return', 'Only the current operator can return control');
    await assert.rejects(ctx.control.change('return'), /Only the current operator/);
    await flush();
    assert.deepEqual(actions(ctx), ['request', 'acquire', 'return']);
  });

  it('retakes again after a later lapse of a renewed hold', async () => {
    await holdConnected(ctx);
    ctx.control.receive({ mode: 'paused', mine: false, revision: ctx.server.revision + 1 });
    await flush();
    mock.timers.tick(HUMAN_CONTROL_MS - CONTROL_RENEW_BEFORE_MS + CONTROL_TICK_MS);
    await flush();
    ctx.control.receive({ mode: 'paused', mine: false, revision: ctx.server.revision + 1 });
    await flush();
    assert.deepEqual(actions(ctx), ['request', 'acquire', 'request', 'acquire', 'renew', 'request', 'acquire']);
    assert.equal(ctx.control.snapshot().interactive, true);
  });

  it('retakes once when the desktop clock runs ahead and every hold looks expired', async () => {
    const granted = answer;
    ctx.server.answer = (message) => {
      const reply: any = granted(message);
      if (reply?.state) reply.state.expiresAt = Date.now() - 1;
      return reply;
    };
    ctx.control.connect({ mode: 'human', mine: true, revision: 1, expiresAt: Date.now() - 1 });
    for (let i = 0; i < 10; i++) {
      mock.timers.tick(CONTROL_TICK_MS);
      await flush();
    }
    assert.deepEqual(actions(ctx), ['request', 'acquire']);
  });

  it('publishes every change to the listener', async () => {
    const before = ctx.snapshots.length;
    await ctx.control.change('acquire');
    assert.ok(ctx.snapshots.length > before);
    assert.equal(ctx.snapshots.at(-1).mode, 'human');
  });
});

describe('drivenElsewhere', () => {
  it('is true only while an agent, or a person on another desktop, drives', () => {
    assert.equal(drivenElsewhere({ mode: 'agent' }), true);
    assert.equal(drivenElsewhere({ mode: 'human', mine: false }), true);
    assert.equal(drivenElsewhere({ mode: 'human', mine: true }), false);
    assert.equal(drivenElsewhere({ mode: 'paused', mine: false }), false);
    assert.equal(drivenElsewhere({ mode: 'disconnected' }), false);
  });
});
