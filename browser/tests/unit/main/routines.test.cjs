/**
 * Unit tests for routines: when each schedule is due, what a saved routine may
 * hold, and that the scheduler runs one routine at a time, only while the
 * browser is free, keeping what the run answered.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { Routines, isDue, validRoutine } = require('../../../main/routines.cjs');
const { ROUTINE_TICK_MS, ROUTINE_RUNS_KEPT } = require('../../../main/constants.cjs');
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

  it('never runs a paused routine', () => {
    assert.equal(isDue(routine({ kind: 'every', n: 1, unit: 'minutes' }, { enabled: false }), EIGHT_AM + HOUR), false);
  });
});

describe('validRoutine', () => {
  const ok = { name: 'Inbox', prompt: 'Check my inbox', schedule: { kind: 'daily', at: '07:30' } };

  it('keeps a named prompt on an every-N or daily schedule', () => {
    assert.equal(validRoutine(ok), true);
    assert.equal(validRoutine({ ...ok, schedule: { kind: 'every', n: 15, unit: 'minutes' } }), true);
  });

  it('refuses a routine without a name or a prompt', () => {
    assert.equal(validRoutine({ ...ok, name: '  ' }), false);
    assert.equal(validRoutine({ ...ok, prompt: '' }), false);
    assert.equal(validRoutine(null), false);
  });

  it('refuses a schedule it cannot keep', () => {
    for (const schedule of [
      { kind: 'every', n: 0, unit: 'minutes' },
      { kind: 'every', n: 1.5, unit: 'hours' },
      { kind: 'every', n: 5, unit: 'seconds' },
      { kind: 'daily', at: '24:00' },
      { kind: 'daily', at: '9:00' },
      { kind: 'cron', at: '* * * * *' },
      { kind: 'constructor' },
      undefined,
    ]) {
      assert.equal(validRoutine({ ...ok, schedule }), false, JSON.stringify(schedule));
    }
  });
});

describe('Routines', () => {
  let ctx, asked, answer, routines;
  const every = { kind: 'every', n: 1, unit: 'minutes' };

  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval', 'Date'], now: EIGHT_AM });
    ctx = mainCtx();
    ctx.config.values = {};
    asked = [];
    answer = async () => ({ text: 'DONE: 3 new emails' });
    routines = new Routines(ctx, (_ctx, _e, messages) => (asked.push(messages), answer()));
  });
  afterEach(() => mock.timers.reset());

  it('saves a routine with only the fields it knows, and tells the shell', () => {
    routines.save({ name: ' Inbox ', prompt: 'Check', schedule: { ...every, x: 1 }, evil: 1 });
    const list = routines.list();
    assert.deepEqual(list, [
      { id: list[0].id, createdAt: EIGHT_AM, name: 'Inbox', prompt: 'Check', schedule: every, enabled: true },
    ]);
    assert.equal(ctx.config.saves, 1);
    assert.equal(ctx.shell.sentOn('routines-changed').length, 1);
  });

  it('edits a routine in place, keeping its history', () => {
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: every }).routines;
    routines.update(saved.id, (r) => ({ ...r, lastRunAt: 5 }));
    const [edited] = routines.save({ id: saved.id, name: 'B', prompt: 'p', schedule: every, enabled: false }).routines;
    assert.deepEqual([edited.id, edited.name, edited.enabled, edited.lastRunAt], [saved.id, 'B', false, 5]);
    assert.equal(routines.list().length, 1);
  });

  it('refuses a routine it cannot keep, saving nothing', () => {
    assert.throws(() => routines.save({ name: 'A', prompt: '', schedule: every }), /needs a name, a prompt/);
    assert.equal(ctx.config.saves, 0);
  });

  it('deletes a routine', () => {
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: every }).routines;
    assert.deepEqual(routines.remove(saved.id).routines, []);
  });

  it('asks the agent the prompt of a routine once it is due, on the tick, and records the run', async () => {
    answer = async () => ({ text: 'DONE: 3 new emails', toolCalls: [{ name: 'navigate' }, { name: 'click' }] });
    routines.save({ name: 'A', prompt: 'Check my inbox', schedule: every });
    routines.start();
    mock.timers.tick(ROUTINE_TICK_MS);
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(asked, [[{ role: 'user', content: 'Check my inbox' }]]);
    const [ran] = routines.list();
    assert.equal(ran.lastRunAt, EIGHT_AM + MINUTE);
    const [run] = ran.runs;
    assert.deepEqual(
      { ...run, id: 'x' },
      {
        id: 'x',
        startedAt: EIGHT_AM + MINUTE,
        finishedAt: EIGHT_AM + MINUTE,
        status: 'done',
        result: 'DONE: 3 new emails',
        steps: ['navigate', 'click'],
      },
    );
    assert.equal(routines.running, null);
  });

  it('shows a run as running while the agent works, and tells the shell at start and end', async () => {
    let finish;
    answer = () => new Promise((resolve) => (finish = resolve));
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: every }).routines;
    const running = routines.runNow(saved.id);
    assert.equal(routines.snapshot().running, saved.id);
    assert.equal(routines.list()[0].runs[0].status, 'running');
    finish({ text: 'DONE' });
    await running;
    assert.equal(routines.list()[0].runs[0].status, 'done');
    assert.equal(ctx.shell.sentOn('routines-changed').length, 3, 'saved, started, finished');
  });

  it('records a failed run with its error, and a stopped one as stopped', async () => {
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: every }).routines;
    answer = async () => ({ error: 'Not connected to server' });
    await routines.runNow(saved.id);
    answer = async () => ({ error: 'Stopped' });
    await routines.runNow(saved.id);
    answer = async () => ({ text: 'FAILED: no such page', failed: true });
    await routines.runNow(saved.id);
    const runs = routines.list()[0].runs.map((r) => [r.status, r.result]);
    assert.deepEqual(runs, [
      ['failed', 'FAILED: no such page'],
      ['stopped', ''],
      ['failed', 'Error: Not connected to server'],
    ]);
  });

  it(`keeps the newest ${ROUTINE_RUNS_KEPT} runs of a routine`, async () => {
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: every }).routines;
    for (let i = 0; i < ROUTINE_RUNS_KEPT + 3; i++) {
      answer = async () => ({ text: `run ${i}` });
      await routines.runNow(saved.id);
    }
    const runs = routines.list()[0].runs;
    assert.equal(runs.length, ROUTINE_RUNS_KEPT);
    assert.equal(runs[0].result, `run ${ROUTINE_RUNS_KEPT + 2}`);
  });

  it('marks a run the app quit in the middle of as interrupted, at the next start', () => {
    ctx.config.values.routines = [
      {
        ...routine(every),
        runs: [
          { id: 'a', status: 'running' },
          { id: 'b', status: 'done' },
        ],
      },
    ];
    routines.start();
    assert.deepEqual(
      routines.list()[0].runs.map((r) => r.status),
      ['interrupted', 'done'],
    );
  });

  it('says when each routine runs next, and nothing for a paused one', () => {
    routines.save({ name: 'A', prompt: 'p', schedule: { kind: 'every', n: 2, unit: 'hours' } });
    routines.save({ name: 'B', prompt: 'p', schedule: every, enabled: false });
    const [a, b] = routines.snapshot().routines;
    assert.deepEqual([a.nextRunAt, b.nextRunAt], [EIGHT_AM + 2 * HOUR, null]);
  });

  it('waits while the browser is busy: offline, in a chat, recording, or running another routine', async () => {
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: every }).routines;
    const later = EIGHT_AM + HOUR;
    const busy = [
      () => (ctx.socket.ready = false),
      () => (ctx.chatAbort = new AbortController()),
      () => (ctx.recorder.recording = true),
      () => (routines.running = 'other'),
    ];
    for (const makeBusy of busy) {
      const undo = { ready: ctx.socket.ready, recording: ctx.recorder.recording };
      makeBusy();
      await routines.tick(later);
      Object.assign(ctx.socket, { ready: undo.ready });
      Object.assign(ctx.recorder, { recording: undo.recording });
      ctx.chatAbort = null;
      routines.running = null;
    }
    assert.deepEqual(asked, []);
    await routines.tick(later);
    assert.equal(asked.length, 1, `${saved.name} runs once the browser is free`);
  });

  it('runs a routine now on demand, even when it is not due', async () => {
    const [saved] = routines.save({ name: 'A', prompt: 'p', schedule: { kind: 'daily', at: '23:00' } }).routines;
    await routines.runNow(saved.id);
    assert.equal(asked.length, 1);
  });
});
