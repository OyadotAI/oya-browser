/**
 * Unit tests for routines (main/routines.cjs): when each schedule is due, and
 * that the app keeps the project's routines in step with the server, claims a
 * run before running it (so two desktops never run the same one), runs one at
 * a time while the browser is free, and records how each run ended. The
 * server is a fake behind fetch.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { Routines, isDue } = require('../../../main/routines.cjs');
const { mainCtx } = require('../support/main-ctx.cjs');

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
/** 2026-09-22 08:00 local time. */
const EIGHT_AM = new Date(2026, 8, 22, 8, 0).getTime();

/** A routine on `schedule`, created at eight and never run. */
const routine = (schedule, extra = {}) => ({ id: 'r1', enabled: true, createdAt: EIGHT_AM, schedule, ...extra });

describe('isDue', () => {
  it('runs an every-N routine N units after its last run, or its creation', () => {
    const every = routine({ kind: 'every', n: 30, unit: 'minutes' });
    assert.equal(isDue(every, EIGHT_AM + 29 * MINUTE), false);
    assert.equal(isDue(every, EIGHT_AM + 30 * MINUTE), true);
    const ran = { ...every, lastRunAt: EIGHT_AM + HOUR };
    assert.equal(isDue(ran, EIGHT_AM + HOUR + 29 * MINUTE), false);
    assert.equal(isDue({ ...every, schedule: { kind: 'every', n: 2, unit: 'hours' } }, EIGHT_AM + 2 * HOUR), true);
  });

  it('runs a daily routine once its time has come today', () => {
    const daily = routine({ kind: 'daily', at: '09:00' });
    assert.equal(isDue(daily, EIGHT_AM + 59 * MINUTE), false);
    assert.equal(isDue(daily, EIGHT_AM + HOUR), true);
  });

  it('runs a daily routine that already ran today again only tomorrow', () => {
    const ran = routine({ kind: 'daily', at: '09:00' }, { lastRunAt: EIGHT_AM + HOUR });
    assert.equal(isDue(ran, EIGHT_AM + 10 * HOUR), false);
    assert.equal(isDue(ran, EIGHT_AM + 25 * HOUR), true);
  });

  it('runs a routine missed while the app was closed once, not once per missed run', () => {
    const missed = routine({ kind: 'every', n: 1, unit: 'hours' });
    const later = EIGHT_AM + 10 * HOUR;
    assert.equal(isDue(missed, later), true);
    assert.equal(isDue({ ...missed, lastRunAt: later }, later + MINUTE), false);
  });

  it('never runs a routine that is off', () => {
    assert.equal(isDue(routine({ kind: 'every', n: 1, unit: 'minutes' }, { enabled: false }), EIGHT_AM + HOUR), false);
  });

  it('never starts a routine another browser is running', () => {
    const running = routine({ kind: 'every', n: 1, unit: 'minutes' }, { runs: [{ id: 'x', status: 'running' }] });
    assert.equal(isDue(running, EIGHT_AM + HOUR), false);
  });
});

/**
 * A fake project server behind fetch: the routines it holds, the calls it got,
 * and an answer per "METHOD path" (a function of the body, or a status to refuse with).
 */
function fakeServer(routines, answers = {}) {
  const calls = [];
  const fetch = mock.method(globalThis, 'fetch', async (url, init = {}) => {
    const method = init.method || 'GET';
    const path = new URL(url).pathname.replace(/^\/api\//, '');
    const body = init.body ? JSON.parse(init.body) : undefined;
    calls.push([method, path, body]);
    const answer = answers[`${method} ${path}`];
    if (typeof answer === 'number')
      return { ok: false, status: answer, json: async () => ({ error: `refused ${answer}` }) };
    const json = answer ? answer(body) : method === 'GET' ? { routines } : {};
    return { ok: true, status: 200, json: async () => json };
  });
  return { calls, fetch, called: (method, path) => calls.filter(([m, p]) => m === method && p === path) };
}

/** A routine as the server holds it. */
const held = (extra = {}) => ({
  id: 'r1',
  name: 'Prices',
  prompt: 'Check prices',
  enabled: true,
  createdAt: EIGHT_AM,
  lastRunAt: null,
  schedule: { kind: 'every', n: 1, unit: 'hours' },
  runs: [],
  ...extra,
});

describe('Routines', () => {
  let ctx;
  beforeEach(() => {
    ctx = mainCtx();
    ctx.config.values = { serverUrl: 'ws://s.test/ws', apiKey: 'k' };
    ctx.recorder = { recording: false };
  });
  afterEach(() => mock.restoreAll());

  it('reads the project’s routines from the server and tells the pane, with when each runs next', async () => {
    fakeServer([held()]);
    const routines = new Routines(ctx);
    const snapshot = await routines.refresh();
    assert.equal(snapshot.routines[0].nextRunAt, EIGHT_AM + HOUR);
    assert.equal(snapshot.online, true);
    assert.equal(ctx.shell.sent.at(-1).channel, 'routines-changed');
  });

  it('hands routines kept locally before the move to the project once, then forgets them here', async () => {
    const server = fakeServer([]);
    ctx.config.values.routines = [held({ id: 'local' })];
    await new Routines(ctx).refresh();
    assert.deepEqual(server.called('POST', 'routines/import')[0][2], { routines: [held({ id: 'local' })] });
    assert.equal(ctx.config.values.routines, undefined);
    await new Routines(ctx).refresh();
    assert.equal(server.called('POST', 'routines/import').length, 1);
  });

  it('claims a due routine, asks the agent its prompt, and records how the run ended', async () => {
    const server = fakeServer([held()]);
    const run = mock.fn(async () => ({ text: 'Cheapest is $4', toolCalls: [{ name: 'navigate' }] }));
    await new Routines(ctx, run).tick(EIGHT_AM + HOUR);
    const [claim] = server.called('POST', 'routines/r1/claim');
    assert.deepEqual([claim[2].lastRunAt, claim[2].browserId], [null, 'b1']);
    assert.equal(run.mock.calls[0].arguments[2][0].content, 'Check prices');
    const [ending] = server.calls.filter(([m, p]) => m === 'PATCH' && p.startsWith('routines/r1/runs/'));
    assert.deepEqual(ending[2], { status: 'done', result: 'Cheapest is $4', steps: ['navigate'] });
  });

  it('lets another desktop have a run it claimed first, without running the agent', async () => {
    fakeServer([held()], { 'POST routines/r1/claim': 409 });
    const run = mock.fn(async () => ({ text: 'x' }));
    await new Routines(ctx, run).tick(EIGHT_AM + HOUR);
    assert.equal(run.mock.callCount(), 0);
  });

  it('records a failed run with its error, and a stopped one as stopped', async () => {
    const server = fakeServer([held()]);
    await new Routines(ctx, async () => ({ error: 'Page did not load' })).tick(EIGHT_AM + HOUR);
    await new Routines(ctx, async () => ({ error: 'Stopped' })).tick(EIGHT_AM + HOUR);
    const endings = server.calls.filter(([m]) => m === 'PATCH').map(([, , body]) => [body.status, body.result]);
    assert.deepEqual(endings, [
      ['failed', 'Error: Page did not load'],
      ['stopped', ''],
    ]);
  });

  it('waits while the browser is busy: offline, in an Ask, recording, or running another routine', async () => {
    const server = fakeServer([held()]);
    const routines = new Routines(ctx, async () => ({ text: 'x' }));
    ctx.chatAbort = new AbortController();
    await routines.tick(EIGHT_AM + HOUR);
    ctx.chatAbort = null;
    ctx.recorder.recording = true;
    await routines.tick(EIGHT_AM + HOUR);
    ctx.recorder.recording = false;
    ctx.socket.ready = false;
    await routines.tick(EIGHT_AM + HOUR);
    assert.equal(server.called('POST', 'routines/r1/claim').length, 0);
  });

  it('says why Run now cannot start, and answers once a run is claimed', async () => {
    fakeServer([held()], { 'POST routines/r1/claim': 409 });
    const routines = new Routines(ctx, () => new Promise(() => {}));
    await routines.refresh();
    ctx.chatAbort = new AbortController();
    assert.match((await routines.runNow('r1')).error, /busy with an Ask/);
    ctx.chatAbort = null;
    assert.equal((await routines.runNow('r1')).error, 'refused 409');
  });

  it('stops only its own run of the routine it was asked to stop', async () => {
    fakeServer([held()]);
    const routines = new Routines(ctx, () => new Promise(() => {}));
    await routines.refresh();
    await routines.runNow('r1');
    ctx.chatAbort = new AbortController();
    assert.equal(routines.stop('other'), false);
    assert.equal(ctx.chatAbort.signal.aborted, false);
    assert.equal(routines.stop('r1'), true);
    assert.equal(ctx.chatAbort.signal.aborted, true);
  });

  it('marks its own runs a quit cut short as interrupted, and leaves another browser’s alone', async () => {
    const runs = [
      { id: 'mine', status: 'running', by: 'b1', startedAt: 1 },
      { id: 'theirs', status: 'running', by: 'b2', startedAt: 1 },
    ];
    const server = fakeServer([held({ runs })]);
    await new Routines(ctx).refresh();
    const patched = server.calls.filter(([m]) => m === 'PATCH').map(([, path, body]) => [path, body.status]);
    assert.deepEqual(patched, [['routines/r1/runs/mine', 'interrupted']]);
  });

  it('creates, edits, turns off, clears and deletes through the server, and says why when offline', async () => {
    const server = fakeServer([held()]);
    const routines = new Routines(ctx);
    const input = { name: 'A', prompt: 'B', schedule: { kind: 'daily', at: '09:00' }, enabled: true };
    await routines.save(input);
    await routines.save({ ...input, id: 'r1' });
    await routines.setEnabled('r1', false);
    await routines.clearHistory('r1');
    await routines.remove('r1');
    const writes = server.calls.filter(([m]) => m !== 'GET').map(([m, p]) => `${m} ${p}`);
    assert.deepEqual(writes, [
      'POST routines',
      'PATCH routines/r1',
      'PATCH routines/r1',
      'DELETE routines/r1/runs',
      'DELETE routines/r1',
    ]);
    ctx.socket.ready = false;
    assert.match((await routines.remove('r1')).error, /Connect to Oya/);
  });

  it('passes on the server’s refusal of a routine', async () => {
    fakeServer([], { 'POST routines': 400 });
    assert.equal((await new Routines(ctx).save({ name: '' })).error, 'refused 400');
  });
});
